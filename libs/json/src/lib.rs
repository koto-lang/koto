//! A Koto language module for working with JSON data

use {
    koto_runtime::{runtime_error, Value, ValueList, ValueMap, ValueVec},
    koto_serialize::SerializableValue,
    serde_json::Value as JsonValue,
};

pub fn json_value_to_koto_value(value: &serde_json::Value) -> Result<Value, String> {
    let result = match value {
        JsonValue::Null => Value::Empty,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => match n.as_i64() {
            Some(n64) => Value::Number(n64.into()),
            None => match n.as_f64() {
                Some(n64) => Value::Number(n64.into()),
                None => return Err(format!("Number is out of range: {}", n)),
            },
        },
        JsonValue::String(s) => Value::Str(s.as_str().into()),
        JsonValue::Array(a) => {
            match a
                .iter()
                .map(|entry| json_value_to_koto_value(entry))
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        JsonValue::Object(o) => {
            let mut map = ValueMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.add_value(key, json_value_to_koto_value(value)?);
            }
            Value::Map(map)
        }
    };

    Ok(result)
}

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("from_string", |vm, args| match vm.get_args(args) {
        [Str(s)] => match serde_json::from_str(s) {
            Ok(value) => match json_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("json.from_string: Error while parsing input: {}", e),
            },
            Err(e) => runtime_error!(
                "json.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        _ => runtime_error!("json.from_string expects a string as argument"),
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [value] => match serde_json::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => runtime_error!("json.to_string: {}", e),
        },
        _ => runtime_error!("json.to_string expects a single argument"),
    });

    result
}
