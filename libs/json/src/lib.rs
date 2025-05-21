//! A Koto language module for working with JSON data

use koto_runtime::prelude::*;
use koto_serde::{DeserializableKValue, SerializableKValue};

pub fn make_module() -> KMap {
    let result = KMap::with_type("json");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match serde_json::from_str::<DeserializableKValue>(s) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!(
                "json.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_json::to_string_pretty(&SerializableKValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("json.to_string: {e}"),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
