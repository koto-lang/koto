//! A Koto language module for working with TOML data

use koto_runtime::prelude::*;
use koto_serde::{DeserializableKValue, SerializableKValue};

pub fn make_module() -> KMap {
    let result = KMap::with_type("toml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match toml::from_str::<DeserializableKValue>(s) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("error while parsing input: {e}"),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match toml::to_string_pretty(&SerializableKValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("toml.to_string: {e}"),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
