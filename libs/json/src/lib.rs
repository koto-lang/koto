//! A Koto language module for working with JSON data

cfg_select! {
    feature = "plugin" => {
        use koto_plugin::prelude::*;
        use koto_serde::plugin::{DeserializableKValue, SerializableKValue};
    }
    _ => {
        use koto_runtime::prelude::*;
        use koto_serde::{DeserializableKValue, SerializableKValue};
    }
}

pub fn make_module() -> KMap {
    let result = KMap::with_type("json");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match serde_json::from_str::<DeserializableKValue>(s) {
            Ok(result) => Ok(result.into()),
            Err(error) => runtime_error!("json.from_string: Error while parsing input: {}", error),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_json::to_string_pretty(&SerializableKValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(error) => runtime_error!(format!("json.to_string: {error}")),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}

#[cfg(feature = "plugin")]
koto_plugin::export_plugin!(make_module);
