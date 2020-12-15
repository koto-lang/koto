//! A Koto language module for working with TOML data

use {
    koto_runtime::{external_error, Value, ValueList, ValueMap, ValueVec},
    koto_serialize::SerializableValue,
    toml::Value as Toml,
};

fn toml_to_koto_value(value: &Toml) -> Result<Value, String> {
    let result = match value {
        Toml::Boolean(b) => Value::Bool(*b),
        Toml::Integer(i) => Value::Number(i.into()),
        Toml::Float(f) => Value::Number(f.into()),
        Toml::String(s) => Value::Str(s.as_str().into()),
        Toml::Array(a) => {
            match a
                .iter()
                .map(|entry| toml_to_koto_value(entry))
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        Toml::Table(o) => {
            let mut map = ValueMap::with_capacity(o.len());
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

    let mut result = ValueMap::new();

    result.add_fn("from_string", |vm, args| match vm.get_args(args) {
        [Str(s)] => match toml::from_str(s) {
            Ok(toml) => match toml_to_koto_value(&toml) {
                Ok(result) => Ok(result),
                Err(e) => external_error!("toml.from_string: Error while parsing input: {}", e),
            },
            Err(e) => external_error!(
                "toml.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        _ => external_error!("toml.from_string expects a string as argument"),
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [value] => match toml::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => external_error!("toml.to_string: {}", e),
        },
        _ => external_error!("toml.to_string expects a single argument"),
    });

    result
}
