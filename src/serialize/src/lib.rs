//! Serde serialization support for Koto value types

use {
    koto_runtime::Value,
    serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer},
};

pub struct SerializableValue<'a>(pub &'a Value);

impl<'a> Serialize for SerializableValue<'a> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Value::Empty => s.serialize_unit(),
            Value::Bool(b) => s.serialize_bool(*b),
            Value::Number(n) => {
                if n.is_f64() {
                    s.serialize_f64(f64::from(n))
                } else {
                    s.serialize_i64(i64::from(n))
                }
            }
            Value::List(l) => {
                let mut seq = s.serialize_seq(Some(l.len()))?;
                for element in l.data().iter() {
                    seq.serialize_element(&SerializableValue(element))?;
                }
                seq.end()
            }
            Value::Tuple(t) => {
                let mut seq = s.serialize_seq(Some(t.data().len()))?;
                for element in t.data().iter() {
                    seq.serialize_element(&SerializableValue(element))?;
                }
                seq.end()
            }
            Value::Map(m) => {
                let mut seq = s.serialize_map(Some(m.data().len()))?;
                for (key, value) in m.data().iter() {
                    seq.serialize_entry(&key.to_string(), &SerializableValue(value))?;
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
