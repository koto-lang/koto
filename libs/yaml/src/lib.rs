//! A Koto language module for working with YAML data

use koto_runtime::prelude::*;
use koto_serde::{DeserializableKValue, SerializableKValue};

pub fn make_module() -> KMap {
    let result = KMap::with_type("yaml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match serde_yaml_ng::from_str::<DeserializableKValue>(s) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("error while parsing input: {e}"),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_yaml_ng::to_string(&SerializableKValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("yaml.to_string: {}", e),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
