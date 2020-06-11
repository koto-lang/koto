use {
    crate::{external_error, single_arg_fn},
    koto_runtime::{value, Value, ValueList, ValueMap, ValueVec},
    serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer},
    serde_json::Value as JsonValue,
    std::sync::Arc,
};

struct SerializableValue<'a>(&'a Value);

impl<'a> Serialize for SerializableValue<'a> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Value::Empty => s.serialize_unit(),
            Value::Bool(b) => s.serialize_bool(*b),
            Value::Number(n) => s.serialize_f64(*n),
            Value::List(l) => {
                let mut seq = s.serialize_seq(Some(l.len()))?;
                for element in l.data().iter() {
                    seq.serialize_element(&SerializableValue(element))?;
                }
                seq.end()
            }
            Value::Map(m) => {
                let mut seq = s.serialize_map(Some(m.data().len()))?;
                for (key, value) in m.data().iter() {
                    seq.serialize_entry(key.as_str(), &SerializableValue(value))?;
                }
                seq.end()
            }
            Value::Str(string) => s.serialize_str(string),
            Value::ExternalValue(value) => s.serialize_str(&value.read().unwrap().to_string()),
            // TODO, is it ok to do nothing for non-fundamental types like Range and Num4?
            _ => s.serialize_unit(),
        }
    }
}

fn json_value_to_koto_value(value: &serde_json::Value) -> Result<Value, String> {
    let result = match value {
        JsonValue::Null => Value::Empty,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => match n.as_f64() {
            Some(n64) => Value::Number(n64),
            None => return Err(format!("Number is out of range for an f64: {}", n)),
        },
        JsonValue::String(s) => Value::Str(Arc::new(s.clone())),
        JsonValue::Array(a) => {
            match a
                .iter()
                .map(|entry| json_value_to_koto_value(entry))
                .collect::<Result<ValueVec, String>>()
            {
                Ok(result) => Value::List(ValueList::with_data(result)),
                Err(e) => return Err(e),
            }
        }
        JsonValue::Object(o) => {
            // Object(Map<String, Value>),
            let mut map = ValueMap::with_capacity(o.len());
            for (key, value) in o.iter() {
                map.add_value(key, json_value_to_koto_value(value)?);
            }
            Value::Map(map)
        }
    };

    Ok(result)
}

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut json = ValueMap::new();

    single_arg_fn!(json, "from_string", Str, s, {
        match serde_json::from_str(&s) {
            Ok(value) => match json_value_to_koto_value(&value) {
                Ok(result) => Ok(result),
                Err(e) => external_error!("json.from_string: Error while parsing input: {}", e),
            },
            Err(e) => external_error!(
                "json.from_string: Error while parsing input: {}",
                e.to_string()
            ),
        }
    });

    json.add_fn("to_string", |_, args| match &args {
        [value] => match serde_json::to_string_pretty(&SerializableValue(value)) {
            Ok(result) => Ok(Str(Arc::new(result))),
            Err(e) => external_error!("json.to_string: {}", e),
        },
        _ => external_error!("number expects a single argument, found {}", args.len()),
    });

    global.add_value("json", Map(json));
}
