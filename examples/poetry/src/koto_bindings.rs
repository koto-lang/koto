use {
    crate::Poetry,
    koto::runtime::{
        make_runtime_error, unexpected_type_error_with_slice, ExternalData, ExternalValue,
        KotoIterator, MetaMap, Value, ValueIterator, ValueIteratorOutput, ValueMap,
    },
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
            unexpected => {
                unexpected_type_error_with_slice("poetry.new", "a String as argument", unexpected)
            }
        }
    });

    result
}

thread_local!(
static POETRY_BINDINGS: Rc<RefCell<MetaMap>> = {
    use Value::{Empty, Iterator, Str};

    let mut bindings = MetaMap::with_type_name("Poetry");

    bindings.add_named_instance_fn_mut(
        "add_source_material",
        |poetry: &mut KotoPoetry, _, args| match args {
            [Str(text)] => {
                poetry.0.add_source_material(text);
                Ok(Empty)
            }
            unexpected => {
                unexpected_type_error_with_slice("poetry.add_source_material", "a String as argument", unexpected)
            }
        },
    );

    bindings.add_named_instance_fn("iter", |_poetry: &KotoPoetry, external_value, _| {
        let iter = PoetryIter {
            poetry: external_value.clone(),
        };
        Ok(Iterator(ValueIterator::new(iter)))
    });

    bindings.add_named_instance_fn_mut("next_word", |poetry: &mut KotoPoetry, _, _| {
        let result = match poetry.0.next_word() {
            Some(word) => Str(word.as_ref().into()),
            None => Empty,
        };
        Ok(result)
    });

    Rc::new(RefCell::new(bindings))
});

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
        use Value::{Empty, Str};

        match self.poetry.data.borrow_mut().downcast_mut::<KotoPoetry>() {
            Some(poetry) => {
                let result = match poetry.0.next_word() {
                    Some(word) => Str(word.as_ref().into()),
                    None => Empty,
                };
                Some(ValueIteratorOutput::Value(result))
            }
            None => Some(ValueIteratorOutput::Error(make_runtime_error!(
                "poetry.iter: Unexpected internal data type"
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
