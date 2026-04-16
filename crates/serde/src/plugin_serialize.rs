use koto_plugin::prelude::{KNumber, KValue, KotoCollection, KotoMap, KotoSlice};
use serde_core::{
    Serialize,
    ser::{self, SerializeMap, SerializeSeq},
};

use crate::Error;

/// A newtype for plugin [KValue] that implements [Serialize](serde_core::Serialize).
pub struct SerializableKValue<'a>(pub &'a KValue);

impl Serialize for SerializableKValue<'_> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        match self.0 {
            KValue::Null => s.serialize_unit(),
            KValue::Bool(b) => s.serialize_bool(*b),
            KValue::Number(KNumber::I64(n)) => s.serialize_i64(*n),
            KValue::Number(KNumber::F64(n)) => s.serialize_f64(*n),
            KValue::List(l) => {
                let mut seq = s.serialize_seq(Some(l.len()))?;
                for element in l.iter() {
                    seq.serialize_element(&SerializableKValue(&element))?;
                }
                seq.end()
            }
            KValue::Tuple(t) => {
                let mut seq = s.serialize_seq(Some(t.len()))?;
                for element in t.iter() {
                    seq.serialize_element(&SerializableKValue(&element))?;
                }
                seq.end()
            }
            KValue::Map(m) => {
                let mut seq = s.serialize_map(Some(m.len()))?;
                for (key, value) in KotoMap::iter(m) {
                    let key = key_to_string(&key).map_err(ser::Error::custom)?;
                    seq.serialize_entry(&key, &SerializableKValue(&value))?;
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

fn key_to_string(value: &KValue) -> Result<String, Error> {
    Ok(match value {
        KValue::Null => "null".to_string(),
        KValue::Bool(value) => value.to_string(),
        KValue::Number(KNumber::I64(value)) => value.to_string(),
        KValue::Number(KNumber::F64(value)) => {
            if value.fract() == 0.0 {
                format!("{value:.1}")
            } else {
                value.to_string()
            }
        }
        KValue::Str(value) => value.to_string(),
        KValue::List(_) => return Err(Error::Unsupported("list keys must be hashable".into())),
        KValue::Tuple(values) => {
            let contents = KotoSlice::iter(values)
                .map(|value| key_to_string(&value))
                .collect::<Result<Vec<_>, _>>()?;
            format!("({})", contents.join(", "))
        }
        KValue::Map(_) => return Err(Error::Unsupported("map keys must be hashable".into())),
        KValue::Range(_) => return Err(Error::Unsupported("ranges".into())),
        KValue::Function(_) => return Err(Error::Unsupported("functions".into())),
        KValue::NativeFunction(_) => {
            return Err(Error::Unsupported("native functions".into()));
        }
        KValue::Iterator(_) => return Err(Error::Unsupported("iterators".into())),
        KValue::Object(_) => return Err(Error::Unsupported("objects".into())),
    })
}
