use {
    crate::Poetry,
    koto::prelude::*,
    std::{cell::RefCell, rc::Rc},
};

pub fn make_module() -> ValueMap {
    let result = ValueMap::new();

    result.add_fn("new", {
        |vm, args| match vm.get_args(args) {
            [Value::Str(text)] => {
                let mut poetry = Poetry::default();
                poetry.add_source_material(text);
                Ok(KotoPoetry::make_external_value(poetry))
            }
            unexpected => type_error_with_slice("a String", unexpected),
        }
    });

    result
}

thread_local! {
    static POETRY_BINDINGS: Rc<RefCell<MetaMap>> = make_poetry_meta_map();
}

fn make_poetry_meta_map() -> Rc<RefCell<MetaMap>> {
    use Value::{Iterator, Null, Str};

    MetaMapBuilder::<KotoPoetry>::new("Poetry")
        .data_fn_with_args_mut("add_source_material", |poetry, args| match args {
            [Str(text)] => {
                poetry.0.add_source_material(text);
                Ok(Null)
            }
            unexpected => type_error_with_slice("a String", unexpected),
        })
        .instance_fn("iter", |poetry_instance| {
            let iter = PoetryIter {
                poetry: poetry_instance.clone(),
            };
            Ok(Iterator(ValueIterator::new(iter)))
        })
        .data_fn_mut("next_word", |poetry| {
            let result = match poetry.0.next_word() {
                Some(word) => Str(word.as_ref().into()),
                None => Null,
            };
            Ok(result)
        })
        .build()
}

#[derive(Clone)]
struct PoetryIter {
    poetry: ExternalValue,
}

impl KotoIterator for PoetryIter {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for PoetryIter {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        use Value::{Null, Str};

        match self.poetry.data_mut::<KotoPoetry>() {
            Some(mut poetry) => {
                let result = match poetry.0.next_word() {
                    Some(word) => Str(word.as_ref().into()),
                    None => Null,
                };
                Some(ValueIteratorOutput::Value(result))
            }
            None => Some(ValueIteratorOutput::Error(make_runtime_error!(
                "Unexpected internal data type"
            ))),
        }
    }
}

#[derive(Debug)]
pub struct KotoPoetry(Poetry);

impl KotoPoetry {
    fn make_external_value(poetry: Poetry) -> Value {
        let result = ExternalValue::with_shared_meta_map(
            KotoPoetry(poetry),
            POETRY_BINDINGS.with(|meta| meta.clone()),
        );

        Value::ExternalValue(result)
    }
}

impl ExternalData for KotoPoetry {}
