//! A Koto language module for working with TOML data

use {koto_runtime::prelude::*, koto_serialize::SerializableValue, toml::Value as Toml};

pub fn toml_to_koto_value(value: &Toml) -> Result<Value, String> {
    let result = match value {
        Toml::Boolean(b) => Value::Bool(*b),
        Toml::Integer(i) => Value::Number(i.into()),
        Toml::Float(f) => Value::Number(f.into()),
        Toml::String(s) => Value::Str(s.as_str().into()),
        Toml::Array(a) => {
            match a
                .iter()
                .map(toml_to_koto_value)
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        Toml::Table(o) => {
            let map = ValueMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.add_value(key, toml_to_koto_value(value)?);
            }
            Value::Map(map)
        }
        Toml::Datetime(dt) => Value::Str(dt.to_string().into()),
    };

    Ok(result)
}

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("from_string", |vm, args| match vm.get_args(args) {
        [Str(s)] => match toml::from_str(s) {
            Ok(toml) => match toml_to_koto_value(&toml) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("toml.from_string: Error while parsing input: {}", e),
            },
            Err(e) => runtime_error!(
                "toml.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        unexpected => type_error_with_slice("toml.from_string", "a String as argument", unexpected),
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [value] => match toml::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => runtime_error!("toml.to_string: {}", e),
        },
        unexpected => type_error_with_slice("toml.to_string", "a Value as argument", unexpected),
    });

    result
}
