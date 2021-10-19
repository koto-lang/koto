use {
    crate::Poetry,
    koto::runtime::{
        make_runtime_error, runtime_error, ExternalData, ExternalIterator, ExternalValue, MetaMap,
        RwLock, Value, ValueIterator, ValueIteratorOutput, ValueMap,
    },
    lazy_static::lazy_static,
    std::{fmt, sync::Arc},
};

pub fn make_module() -> ValueMap {
    let mut result = ValueMap::new();

    result.add_fn("new", {
        |vm, args| match vm.get_args(args) {
            [Value::Str(text)] => {
                let mut poetry = Poetry::default();
                poetry.add_source_material(text);
                Ok(KotoPoetry::make_external_value(poetry))
            }
            [unexpected] => runtime_error!(
                "poetry.new: Expected a String as argument, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("poetry.new: Expected a String as argument"),
        }
    });

    result
}

lazy_static! {
    static ref POETRY_BINDINGS: Arc<RwLock<MetaMap>> = {
        use Value::{Empty, Iterator, Str};

        let mut bindings = MetaMap::with_type_name("Poetry");

        bindings.add_named_instance_fn_mut(
            "add_source_material",
            |poetry: &mut KotoPoetry, _, args| match args {
                [Str(text)] => {
                    poetry.0.add_source_material(text);
                    Ok(Empty)
                }
                _ => runtime_error!("poetry.add_source_material: Expected a String as argument"),
            },
        );

        bindings.add_named_instance_fn("iter", |_poetry: &KotoPoetry, external_value, _| {
            let iter = PoetryIter {
                poetry: external_value.clone(),
            };
            Ok(Iterator(ValueIterator::make_external(iter)))
        });

        bindings.add_named_instance_fn_mut("next_word", |poetry: &mut KotoPoetry, _, _| {
            let result = match poetry.0.next_word() {
                Some(word) => Str(word.as_ref().into()),
                None => Empty,
            };
            Ok(result)
        });

        Arc::new(RwLock::new(bindings))
    };
}

#[derive(Clone)]
struct PoetryIter {
    poetry: ExternalValue,
}

impl ExternalIterator for PoetryIter {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }
}

impl Iterator for PoetryIter {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        use Value::{Empty, Str};

        match self.poetry.data.write().downcast_mut::<KotoPoetry>() {
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
        let result =
            ExternalValue::with_shared_meta_map(KotoPoetry(poetry), POETRY_BINDINGS.clone());

        Value::ExternalValue(result)
    }
}

impl ExternalData for KotoPoetry {
    fn value_type(&self) -> String {
        "Poetry".to_string()
    }
}

impl fmt::Display for KotoPoetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Poetry")
    }
}
