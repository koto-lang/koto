//! A Koto language module for working with TOML data

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
    let result = KMap::with_type("toml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match toml::from_str::<DeserializableKValue>(s) {
            Ok(result) => Ok(result.into()),
            Err(error) => runtime_error!(format!("error while parsing input: {error}")),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match toml::to_string_pretty(&SerializableKValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(error) => runtime_error!(format!("toml.to_string: {error}")),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}

#[cfg(feature = "plugin")]
koto_plugin::export_plugin!(make_module);
