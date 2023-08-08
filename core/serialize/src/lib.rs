//! Serde serialization support for Koto value types

use koto_runtime::Value;
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// A newtype that allows us to implement support for Serde serialization
pub struct SerializableValue<'a>(pub &'a Value);

impl<'a> Serialize for SerializableValue<'a> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Value::Null => s.serialize_unit(),
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
                let mut seq = s.serialize_seq(Some(t.len()))?;
                for element in t.iter() {
                    seq.serialize_element(&SerializableValue(element))?;
                }
                seq.end()
            }
            Value::Map(m) => {
                let mut seq = s.serialize_map(Some(m.len()))?;
                for (key, value) in m.data().iter() {
                    seq.serialize_entry(&key.to_string(), &SerializableValue(value))?;
                }
                seq.end()
            }
            Value::Str(string) => s.serialize_str(string),
            // TODO, is it ok to do nothing for non-fundamental types, e.g. External Values?
            _ => s.serialize_unit(),
        }
    }
}
