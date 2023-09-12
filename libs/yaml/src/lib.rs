//! A Koto language module for working with YAML data

use koto_runtime::prelude::*;
use koto_serialize::SerializableValue;
use serde_yaml::Value as YamlValue;

pub fn yaml_value_to_koto_value(value: &serde_yaml::Value) -> Result<Value, RuntimeError> {
    let result = match value {
        YamlValue::Null => Value::Null,
        YamlValue::Bool(b) => Value::Bool(*b),
        YamlValue::Number(n) => match n.as_i64() {
            Some(n64) => Value::Number(n64.into()),
            None => match n.as_f64() {
                Some(n64) => Value::Number(n64.into()),
                None => return runtime_error!("Number is out of range: {n}"),
            },
        },
        YamlValue::String(s) => Value::Str(s.as_str().into()),
        YamlValue::Sequence(sequence) => {
            match sequence
                .iter()
                .map(yaml_value_to_koto_value)
                .collect::<Result<ValueVec, RuntimeError>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        YamlValue::Mapping(mapping) => {
            let map = ValueMap::with_capacity(mapping.len());
            for (key, value) in mapping.iter() {
                let key_as_koto_value = yaml_value_to_koto_value(key)?;
                map.insert(
                    ValueKey::try_from(key_as_koto_value)?,
                    yaml_value_to_koto_value(value)?,
                );
            }
            Value::Map(map)
        }
    };

    Ok(result)
}

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("yaml");

    result.add_fn("from_string", |ctx| match ctx.args() {
        [Str(s)] => match serde_yaml::from_str(s) {
            Ok(value) => match yaml_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("Error while parsing input: {}", e),
            },
            Err(e) => runtime_error!("Error while parsing input: {}", e.to_string()),
        },
        unexpected => type_error_with_slice("a String as argument", unexpected),
    });

    result.add_fn("to_string", |ctx| match ctx.args() {
        [value] => match serde_yaml::to_string(&SerializableValue(value)) {
            Ok(result) => Ok(result.into()),
            Err(e) => runtime_error!("yaml.to_string: {}", e),
        },
        unexpected => type_error_with_slice("a Value as argument", unexpected),
    });

    result
}
