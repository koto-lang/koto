use koto_runtime::KValue;
use serde_core::{
    Serialize,
    ser::{self, SerializeMap, SerializeSeq},
};

use crate::Error;

/// A newtype for [KValue] that implements [Serialize](serde_core::Serialize).
pub struct SerializableKValue<'a>(pub &'a KValue);

impl Serialize for SerializableKValue<'_> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
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
            KValue::Object(o) => {
                let serialized_object = o
                    .try_borrow()
                    .map_err(|e| {
                        ser::Error::custom(Error::FailedToSerializeKObject(e.to_string()))
                    })?
                    .serialize()
                    .map_err(|e| {
                        ser::Error::custom(Error::FailedToSerializeKObject(e.to_string()))
                    })?;

                SerializableKValue(&serialized_object).serialize(s)
            }
            other => Err(ser::Error::custom(format!(
                "serialization isn't supported for '{}'",
                other.type_as_string(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_koto_value, to_koto_value};
    use koto_runtime::prelude::*;
    use serde::{Deserialize, Serialize};

    #[test]
    fn object_to_kvalue() {
        // TestObject could return any kind of value from `KotoObject::serialize`,
        // but it defers to `to_koto_value` which produces a map.
        match KotoObject::serialize(&TestObject { x: 123 }).unwrap() {
            KValue::Map(m) => match m.get("x").unwrap() {
                KValue::Number(n) => assert_eq!(n, 123),
                unexpected => unexpected_type("number", &unexpected).unwrap(),
            },
            unexpected => unexpected_type("map", &unexpected).unwrap(),
        }
    }

    #[test]
    fn kvalue_to_object() {
        let kvalue = KValue::Object(KObject::from(TestObject { x: 99 }));
        let test_object: TestObject = from_koto_value(kvalue).unwrap();
        assert_eq!(test_object.x, 99);
    }

    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    struct TestObject {
        x: i64,
    }

    impl KotoType for TestObject {
        fn type_static() -> &'static str
        where
            Self: Sized,
        {
            "TestObject"
        }

        fn type_string(&self) -> KString {
            Self::type_static().into()
        }
    }

    impl KotoCopy for TestObject {
        fn copy(&self) -> KObject {
            (*self).into()
        }
    }

    impl KotoAccess for TestObject {}

    impl KotoObject for TestObject {
        fn serialize(&self) -> koto_runtime::Result<KValue> {
            // Convert this TestObject into a serializable kvalue by calling `to_koto_value`
            to_koto_value(self).map_err(|e| koto_runtime::Error::from(e.to_string()))
        }
    }
}
