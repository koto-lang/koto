//! A Koto language module for working with YAML data

use koto_runtime::{Result, prelude::*};
use koto_serialize::SerializableValue;
use serde_yaml_ng::Value as YamlValue;

pub fn yaml_value_to_koto_value(value: &YamlValue) -> Result<KValue> {
    let result = match value {
        YamlValue::Null => KValue::Null,
        YamlValue::Bool(b) => KValue::Bool(*b),
        YamlValue::Number(n) => match n.as_i64() {
            Some(n64) => KValue::Number(n64.into()),
            None => match n.as_f64() {
                Some(n64) => KValue::Number(n64.into()),
                None => return runtime_error!("number is out of range: {n}"),
            },
        },
        YamlValue::String(s) => KValue::Str(s.as_str().into()),
        YamlValue::Sequence(sequence) => {
            match sequence
                .iter()
                .map(yaml_value_to_koto_value)
                .collect::<Result<Vec<_>>>()
            {
                Ok(result) => KValue::Tuple(result.into()),
                Err(e) => return Err(e),
            }
        }
        YamlValue::Mapping(mapping) => {
            let map = KMap::with_capacity(mapping.len());
            for (key, value) in mapping.iter() {
                let key_as_koto_value = yaml_value_to_koto_value(key)?;
                map.insert(
                    ValueKey::try_from(key_as_koto_value)?,
                    yaml_value_to_koto_value(value)?,
                );
            }
            KValue::Map(map)
        }
        YamlValue::Tagged(tagged_value) => {
            let map = KMap::with_type("TaggedValue");

            let tag = tagged_value.tag.to_string();
            let tag = match tag.strip_prefix("!") {
                Some(stripped) => stripped.to_string(),
                None => tag,
            };

            map.insert("tag", tag);
            map.insert("value", yaml_value_to_koto_value(value)?);

            KValue::Map(map)
        }
    };

    Ok(result)
}

pub fn make_module() -> KMap {
    let result = KMap::with_type("yaml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [KValue::Str(s)] => match serde_yaml_ng::from_str(s) {
            Ok(value) => match yaml_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("error while parsing input: {}", e),
            },
            Err(e) => runtime_error!("error while parsing input: {}", e.to_string()),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_yaml_ng::to_string(&SerializableValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("yaml.to_string: {}", e),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result
}
