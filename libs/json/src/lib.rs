//! A Koto language module for working with JSON data

use koto_runtime::{prelude::*, Result};
use koto_serialize::SerializableValue;
use serde_json::Value as JsonValue;

pub fn json_value_to_koto_value(value: &serde_json::Value) -> Result<KValue> {
    let result = match value {
        JsonValue::Null => KValue::Null,
        JsonValue::Bool(b) => KValue::Bool(*b),
        JsonValue::Number(n) => match n.as_i64() {
            Some(n64) => KValue::Number(n64.into()),
            None => match n.as_f64() {
                Some(n64) => KValue::Number(n64.into()),
                None => return runtime_error!("Number is out of range: {n}"),
            },
        },
        JsonValue::String(s) => KValue::Str(s.as_str().into()),
        JsonValue::Array(a) => {
            match a
                .iter()
                .map(json_value_to_koto_value)
                .collect::<Result<Vec<_>>>()
            {
                Ok(result) => KValue::Tuple(result.into()),
                Err(e) => return Err(e),
            }
        }
        JsonValue::Object(o) => {
            let map = KMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.insert(key.as_str(), json_value_to_koto_value(value)?);
            }
            KValue::Map(map)
        }
    };

    Ok(result)
}

pub fn make_module() -> KMap {
    let result = KMap::with_type("json");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match serde_json::from_str(s) {
            Ok(value) => match json_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("json.from_string: Error while parsing input: {e}"),
            },
            Err(e) => runtime_error!(
                "json.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_json::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("json.to_string: {e}"),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
