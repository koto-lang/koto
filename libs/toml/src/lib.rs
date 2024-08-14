//! A Koto language module for working with TOML data

use koto_runtime::{prelude::*, Result};
use koto_serialize::SerializableValue;
use toml::Value as Toml;

pub fn toml_to_koto_value(value: &Toml) -> Result<KValue> {
    let result = match value {
        Toml::Boolean(b) => KValue::Bool(*b),
        Toml::Integer(i) => KValue::Number(i.into()),
        Toml::Float(f) => KValue::Number(f.into()),
        Toml::String(s) => KValue::Str(s.as_str().into()),
        Toml::Array(a) => match a.iter().map(toml_to_koto_value).collect::<Result<Vec<_>>>() {
            Ok(result) => KValue::Tuple(result.into()),
            Err(e) => return Err(e),
        },
        Toml::Table(o) => {
            let map = KMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.insert(key.as_str(), toml_to_koto_value(value)?);
            }
            KValue::Map(map)
        }
        Toml::Datetime(dt) => KValue::Str(dt.to_string().into()),
    };

    Ok(result)
}

pub fn make_module() -> KMap {
    let result = KMap::with_type("toml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match toml::from_str(s) {
            Ok(toml) => match toml_to_koto_value(&toml) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("Error while parsing input: {e}"),
            },
            Err(e) => runtime_error!("Error while parsing input: {}", e.to_string()),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match toml::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("toml.to_string: {e}"),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
