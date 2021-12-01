//! A Koto language module for working with YAML data

use {
    koto_runtime::{
        runtime_error, unexpected_type_error_with_slice, Value, ValueList, ValueMap, ValueVec,
    },
    koto_serialize::SerializableValue,
    serde_yaml::Value as YamlValue,
};

pub fn yaml_value_to_koto_value(value: &serde_yaml::Value) -> Result<Value, String> {
    let result = match value {
        YamlValue::Null => Value::Empty,
        YamlValue::Bool(b) => Value::Bool(*b),
        YamlValue::Number(n) => match n.as_i64() {
            Some(n64) => Value::Number(n64.into()),
            None => match n.as_f64() {
                Some(n64) => Value::Number(n64.into()),
                None => return Err(format!("Number is out of range: {}", n)),
            },
        },
        YamlValue::String(s) => Value::Str(s.as_str().into()),
        YamlValue::Sequence(sequence) => {
            match sequence
                .iter()
                .map(|entry| yaml_value_to_koto_value(entry))
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        YamlValue::Mapping(mapping) => {
            let mut map = ValueMap::with_capacity(mapping.len());
            for (key, value) in mapping.iter() {
                let key_as_koto_value = yaml_value_to_koto_value(key)?;
                if !key_as_koto_value.is_immutable() {
                    return Err(format!(
                        "Invalid value type for map key: {}",
                        key_as_koto_value.type_as_string()
                    ));
                }
                map.insert(key_as_koto_value.into(), yaml_value_to_koto_value(value)?);
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
        [Str(s)] => match serde_yaml::from_str(s) {
            Ok(value) => match yaml_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => runtime_error!("yaml.from_string: Error while parsing input: {}", e),
            },
            Err(e) => runtime_error!(
                "yaml.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        },
        unexpected => {
            unexpected_type_error_with_slice("yaml.from_string", "a String as argument", unexpected)
        }
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [value] => match serde_yaml::to_string(&SerializableValue(value)) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => runtime_error!("yaml.to_string: {}", e),
        },
        unexpected => {
            unexpected_type_error_with_slice("yaml.to_string", "a Value as argument", unexpected)
        }
    });

    result
}
