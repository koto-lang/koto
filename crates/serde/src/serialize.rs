use koto_runtime::KValue;
use serde::{
    Serialize,
    ser::{self, SerializeMap, SerializeSeq},
};

/// A newtype for [`KValue`] that implements [`serde::Serialize`]
pub struct SerializableKValue<'a>(pub &'a KValue);

impl Serialize for SerializableKValue<'_> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self.0 {
            KValue::Null => s.serialize_unit(),
            KValue::Bool(b) => s.serialize_bool(*b),
            KValue::Number(n) => {
                if n.is_f64() {
                    s.serialize_f64(f64::from(n))
                } else {
                    s.serialize_i64(i64::from(n))
                }
            }
            KValue::List(l) => {
                let mut seq = s.serialize_seq(Some(l.len()))?;
                for element in l.data().iter() {
                    seq.serialize_element(&SerializableKValue(element))?;
                }
                seq.end()
            }
            KValue::Tuple(t) => {
                let mut seq = s.serialize_seq(Some(t.len()))?;
                for element in t.iter() {
                    seq.serialize_element(&SerializableKValue(element))?;
                }
                seq.end()
            }
            KValue::Map(m) => {
                let mut seq = s.serialize_map(Some(m.len()))?;
                for (key, value) in m.data().iter() {
                    seq.serialize_entry(&key.to_string(), &SerializableKValue(value))?;
                }
                seq.end()
            }
            KValue::Str(string) => s.serialize_str(string),
            other => Err(ser::Error::custom(format!(
                "serialization isn't supported for '{}'",
                other.type_as_string(),
            ))),
        }
    }
}
