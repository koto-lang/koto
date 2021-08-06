use {
    crate::Poetry,
    koto::runtime::{
        runtime_error, ExternalData, ExternalValue, MetaMap, RwLock, Value, ValueIterator,
        ValueIteratorOutput, ValueMap,
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
                poetry.add_links(text);
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
        use Value::{Iterator, Str, Empty};

        let mut bindings = MetaMap::with_type_name("Poetry");

        bindings.add_named_instance_fn_mut(
            "add_links", |poetry: &mut KotoPoetry, _, args| match args {
            [Str(text)] => {
                poetry.0.add_links(text);
                Ok(Empty)
            }
            _ => runtime_error!("poetry.add_links: Expected a String as argument"),
        });

        bindings.add_named_instance_fn(
            "iter",
            |_poetry: &KotoPoetry, external_value, _| {
                let poetry_arc = external_value.data.clone();
                let iter = move || {
                    // For each iteration, get the KotoPoetry instance from of the external value.
                    match poetry_arc.write().downcast_mut::<KotoPoetry>() {
                        Some(poetry)=>{
                            let result = match poetry.0.next_word() {
                                Some(word) => Str(word.as_ref().into()),
                                None => Empty,
                            };
                            Some(Ok(ValueIteratorOutput::Value(result)))
                        }
                        None => Some(runtime_error!("poetry.iter - Unexpected internal data type")),
                    }
                };

                // Return an 'external' iterator that will call the above function on each iteration
                Ok(Iterator(ValueIterator::make_external(iter)))
            },
        );

        bindings.add_named_instance_fn_mut( "next_word", |poetry: &mut KotoPoetry, _, _| {
            let result = match poetry.0.next_word() {
                Some(word) => Str(word.as_ref().into()),
                None => Empty,
            };
            Ok(result)
        });

        Arc::new(RwLock::new(bindings))
    };
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
