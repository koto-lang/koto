use {
    crate::{external_error, serializable_value::SerializableValue, single_arg_fn},
    koto_runtime::{value, Value, ValueList, ValueMap, ValueVec},
    std::sync::Arc,
    toml::Value as Toml,
};

fn toml_to_koto_value(value: &Toml) -> Result<Value, String> {
    let result = match value {
        Toml::Boolean(b) => Value::Bool(*b),
        Toml::Integer(i) => {
            let result = *i as f64;
            if result as i64 == *i {
                Value::Number(result)
            } else {
                return Err(format!("Number is out of range for an f64: {}", i));
            }
        }
        Toml::Float(f) => Value::Number(*f),
        Toml::String(s) => Value::Str(Arc::new(s.clone())),
        Toml::Array(a) => {
            match a
                .iter()
                .map(|entry| toml_to_koto_value(entry))
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        Toml::Table(o) => {
            let mut map = ValueMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.add_value(key, toml_to_koto_value(value)?);
            }
            Value::Map(map)
        }
        Toml::Datetime(dt) => Value::Str(Arc::new(dt.to_string())),
    };

    Ok(result)
}

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut toml = ValueMap::new();

    single_arg_fn!(toml, "from_string", Str, s, {
        match toml::from_str(s) {
            Ok(toml) => match toml_to_koto_value(&toml) {
                Ok(result) => Ok(result),
                Err(e) => external_error!("toml.from_string: Error while parsing input: {}", e),
            },
            Err(e) => external_error!(
                "toml.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        }
    });

    toml.add_fn("to_string", |_, args| match &args {
        [value] => match toml::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(Str(Arc::new(result))),
            Err(e) => external_error!("toml.to_string: {}", e),
        },
        _ => external_error!("number expects a single argument, found {}", args.len()),
    });

    global.add_value("toml", Map(toml));
}
