use {
    crate::Poetry,
    koto::runtime::{
        get_external_instance, is_external_instance, runtime_error, visit_external_value,
        ExternalValue, Value, ValueIterator, ValueIteratorOutput, ValueMap,
    },
    std::fmt,
};

pub fn make_module() -> ValueMap {
    use Value::{Map, Str};

    let mut result = ValueMap::new();

    result.add_fn("new", {
        |vm, args| match vm.get_args(args) {
            [Str(text)] => {
                let mut poetry = Poetry::default();
                poetry.add_links(text);
                Ok(Map(KotoPoetry::make_value_map(poetry)))
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

#[derive(Debug)]
pub struct KotoPoetry(Poetry);

impl KotoPoetry {
    fn make_value_map(poetry: Poetry) -> ValueMap {
        use Value::*;

        let mut result = ValueMap::default();

        result.add_instance_fn("add_links", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "poetry", "next_word", Self, poetry, {
                match args {
                    [_, Str(text)] => {
                        poetry.0.add_links(text);
                        Ok(Empty)
                    }
                    _ => runtime_error!("poetry.add_links: Expected a String as argument"),
                }
            })
        });

        result.add_instance_fn("iter", |vm, args| match vm.get_args(args) {
            [Map(poetry_map)] => {
                if is_external_instance::<KotoPoetry>(poetry_map) {
                    let poetry_map = poetry_map.clone();

                    let iter = move || match visit_external_value(
                        &poetry_map,
                        |poetry: &mut KotoPoetry| {
                            let result = poetry
                                .0
                                .next_word()
                                .map_or(Empty, |word| Str(word.as_ref().into()));
                            Ok(result)
                        },
                    ) {
                        Ok(result) => Some(Ok(ValueIteratorOutput::Value(result))),
                        Err(error) => Some(Err(error)),
                    };

                    Ok(Iterator(ValueIterator::make_external(iter)))
                } else {
                    runtime_error!("poetry.iter: Missing Poetry instance")
                }
            }
            _ => runtime_error!("poetry.iter: Expected Poetry instance as argument"),
        });

        result.add_instance_fn("next_word", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "poetry", "next_word", Self, poetry, {
                let result = match poetry.0.next_word() {
                    Some(word) => Str(word.as_ref().into()),
                    None => Empty,
                };
                Ok(result)
            })
        });

        result.insert(
            Value::ExternalDataId.into(),
            Value::make_external_value(Self(poetry)),
        );
        result
    }
}

impl ExternalValue for KotoPoetry {
    fn value_type(&self) -> String {
        "Poetry".to_string()
    }
}

impl fmt::Display for KotoPoetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Poetry")
    }
}
