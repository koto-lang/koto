use crate::{Error, Result};
use koto_runtime::prelude::*;
use serde::{
    Serialize,
    ser::{self, SerializeSeq},
};

/// Serializes a value into a [KValue]
pub fn to_koto_value<T: Serialize>(value: T) -> Result<KValue> {
    value.serialize(Serializer)
}

/// A serializer that produces [KValue]s
pub struct Serializer;

impl serde::Serializer for Serializer {
    type Ok = KValue;

    type Error = Error;

    type SerializeSeq = SerializeTuple;
    type SerializeTuple = SerializeTuple;
    type SerializeTupleStruct = SerializeTuple;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeMapVariant;

    fn serialize_bool(self, v: bool) -> Result<KValue> {
        Ok(v.into())
    }

    fn serialize_i8(self, v: i8) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_i64(self, v: i64) -> Result<KValue> {
        Ok(KValue::Number(v.into()))
    }

    fn serialize_i128(self, v: i128) -> Result<KValue> {
        match i64::try_from(v) {
            Ok(n) => self.serialize_i64(n),
            Err(_) => Err(Error::OutOfRangeI128(v)),
        }
    }

    fn serialize_u8(self, v: u8) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<KValue> {
        self.serialize_i64(v.into())
    }

    fn serialize_u64(self, v: u64) -> Result<KValue> {
        match i64::try_from(v) {
            Ok(n) => self.serialize_i64(n),
            Err(_) => Err(Error::OutOfRangeU64(v)),
        }
    }

    fn serialize_u128(self, v: u128) -> Result<KValue> {
        match i64::try_from(v) {
            Ok(n) => self.serialize_i64(n),
            Err(_) => Err(Error::OutOfRangeU128(v)),
        }
    }

    fn serialize_f32(self, v: f32) -> Result<KValue> {
        self.serialize_f64(v.into())
    }

    fn serialize_f64(self, v: f64) -> Result<KValue> {
        Ok(KValue::Number(v.into()))
    }

    fn serialize_char(self, v: char) -> Result<KValue> {
        self.serialize_str(v.encode_utf8(&mut [0u8; 4]))
    }

    fn serialize_str(self, v: &str) -> Result<KValue> {
        Ok(KValue::Str(v.into()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<KValue> {
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for b in v {
            seq.serialize_element(b)?;
        }
        seq.end()
    }

    fn serialize_none(self) -> Result<KValue> {
        Ok(KValue::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<KValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<KValue> {
        Ok(KValue::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<KValue> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<KValue> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<KValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<KValue>
    where
        T: ?Sized + Serialize,
    {
        let mut data = ValueMap::default();
        data.insert(variant.into(), value.serialize(self)?);
        Ok(KValue::Map(data.into()))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.serialize_tuple(len.unwrap_or_default())
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        Ok(SerializeTuple::with_capacity(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(SerializeTupleVariant::new(variant.into(), len))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(SerializeMap::with_capacity(len.unwrap_or_default()))
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(SerializeMapVariant::new(variant.into(), len))
    }
}

pub struct SerializeTuple {
    elements: Vec<KValue>,
}

impl SerializeTuple {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            elements: Vec::with_capacity(capacity),
        }
    }
}

impl ser::SerializeSeq for SerializeTuple {
    type Ok = KValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.elements.push(value.serialize(Serializer)?);
        Ok(())
    }

    fn end(self) -> Result<KValue> {
        Ok(KValue::Tuple(self.elements.into()))
    }
}

impl ser::SerializeTuple for SerializeTuple {
    type Ok = KValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<KValue> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SerializeTuple {
    type Ok = KValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<KValue> {
        ser::SerializeSeq::end(self)
    }
}

pub struct SerializeTupleVariant {
    name: KString,
    elements: Vec<KValue>,
}

impl SerializeTupleVariant {
    fn new(name: KString, capacity: usize) -> Self {
        Self {
            name,
            elements: Vec::with_capacity(capacity),
        }
    }
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = KValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.elements.push(value.serialize(Serializer)?);
        Ok(())
    }

    fn end(self) -> Result<KValue> {
        let mut data = ValueMap::default();
        data.insert(self.name.into(), KValue::Tuple(self.elements.into()));
        Ok(KValue::Map(data.into()))
    }
}

pub struct SerializeMap {
    entries: ValueMap,
    next_key: Option<ValueKey>,
}

impl SerializeMap {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: ValueMap::with_capacity(capacity),
            next_key: None,
        }
    }
}

impl ser::SerializeMap for SerializeMap {
    type Ok = KValue;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.next_key = Some(
            key.serialize(Serializer)?
                .try_into()
                .map_err(|e: koto_runtime::Error| Error::Message(e.to_string()))?,
        );
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let key = self.next_key.take().ok_or(Error::MissingMapKey)?;
        let value = value.serialize(Serializer)?;
        self.entries.insert(key, value);
        Ok(())
    }

    fn end(self) -> Result<KValue> {
        Ok(KValue::Map(self.entries.into()))
    }
}

impl ser::SerializeStruct for SerializeMap {
    type Ok = KValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<KValue> {
        ser::SerializeMap::end(self)
    }
}

pub struct SerializeMapVariant {
    name: KString,
    entries: ValueMap,
}

impl SerializeMapVariant {
    fn new(name: KString, capacity: usize) -> Self {
        Self {
            name,
            entries: ValueMap::with_capacity(capacity),
        }
    }
}

impl ser::SerializeStructVariant for SerializeMapVariant {
    type Ok = KValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.entries
            .insert(key.into(), value.serialize(Serializer)?);
        Ok(())
    }

    fn end(self) -> Result<KValue> {
        let mut data = ValueMap::default();
        data.insert(self.name.into(), KValue::Map(self.entries.into()));
        Ok(KValue::Map(data.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestStruct {
        n: i64,
        flags: Vec<bool>,
    }

    #[derive(Serialize)]
    enum TestEnum {
        A,
        B(u32),
        C(String, String),
        D {
            maybe_float: Option<f64>,
            tuple: (bool, char, Option<String>),
        },
    }

    #[track_caller]
    fn expected_type(expected: &str, unexpected: &KValue) {
        panic!("expected {expected}, found {}", unexpected.type_as_string());
    }

    #[test]
    fn serialize_struct() -> Result<()> {
        let t = TestStruct {
            n: 123,
            flags: vec![true, false],
        };

        match to_koto_value(t)? {
            KValue::Map(map) => {
                assert_eq!(map.len(), 2);
                match map.get("n") {
                    Some(KValue::Number(n)) => assert_eq!(n, 123),
                    Some(other) => expected_type("number", &other),
                    None => panic!("no entry found for 'n'"),
                }
                match map.get("flags") {
                    Some(KValue::Tuple(t)) => {
                        assert_eq!(t.len(), 2);
                        match &t[0] {
                            KValue::Bool(b) => assert!(*b),
                            other => expected_type("bool", other),
                        }
                        match &t[1] {
                            KValue::Bool(b) => assert!(!*b),
                            other => expected_type("bool", other),
                        }
                    }
                    Some(other) => expected_type("number ", &other),
                    None => panic!("no entry found for 'n'"),
                }
            }
            other => expected_type("map", &other),
        }

        Ok(())
    }

    #[test]
    fn serialize_enum_a() -> Result<()> {
        match to_koto_value(TestEnum::A)? {
            KValue::Str(a) => assert_eq!(a, "A"),
            other => expected_type("string", &other),
        }

        Ok(())
    }

    #[test]
    fn serialize_enum_b() -> Result<()> {
        match to_koto_value(TestEnum::B(99))? {
            KValue::Map(map) => {
                assert_eq!(map.len(), 1);
                match map.get("B") {
                    Some(KValue::Number(n)) => assert_eq!(n, 99),
                    Some(other) => expected_type("number", &other),
                    None => panic!("no entry found for 'B'"),
                }
            }
            other => expected_type("map", &other),
        }

        Ok(())
    }

    #[test]
    fn serialize_enum_c() -> Result<()> {
        match to_koto_value(TestEnum::C("abc".into(), "xyz".into()))? {
            KValue::Map(map) => {
                assert_eq!(map.len(), 1);
                match map.get("C") {
                    Some(KValue::Tuple(t)) => {
                        assert_eq!(t.len(), 2);
                        match &t[0] {
                            KValue::Str(s) => assert_eq!(*s, "abc"),
                            other => expected_type("String", other),
                        }
                        match &t[1] {
                            KValue::Str(s) => assert_eq!(*s, "xyz"),
                            other => expected_type("String", other),
                        }
                    }
                    Some(other) => expected_type("tuple", &other),
                    None => panic!("no entry found for 'C'"),
                }
            }
            other => expected_type("map", &other),
        }

        Ok(())
    }

    #[test]
    fn serialize_enum_d() -> Result<()> {
        match to_koto_value(TestEnum::D {
            maybe_float: Some(42.0),
            tuple: (true, 'x', None),
        })? {
            KValue::Map(map) => {
                assert_eq!(map.len(), 1);
                match map.get("D") {
                    Some(KValue::Map(fields)) => {
                        assert_eq!(fields.len(), 2);
                        match fields.get("maybe_float") {
                            Some(KValue::Number(n)) => assert_eq!(n, 42.0),
                            Some(other) => expected_type("number", &other),
                            None => panic!("no entry found for 'maybe_float'"),
                        }
                        match fields.get("tuple") {
                            Some(KValue::Tuple(t)) => {
                                assert_eq!(t.len(), 3);
                                match &t[0] {
                                    KValue::Bool(b) => assert!(*b),
                                    other => expected_type("bool", other),
                                }
                                match &t[1] {
                                    KValue::Str(s) => assert_eq!(*s, "x"),
                                    other => expected_type("string", other),
                                }
                                match &t[2] {
                                    KValue::Null => {}
                                    other => expected_type("null", other),
                                }
                            }
                            Some(other) => expected_type("number ", &other),
                            None => panic!("no entry found for 'tuple'"),
                        }
                    }
                    Some(other) => expected_type("map", &other),
                    None => panic!("no entry found for 'D'"),
                }
            }
            other => expected_type("map", &other),
        }

        Ok(())
    }
}
