use crate::{Error, Result};
use koto_runtime::{KMap, KNumber, KValue};
use serde::{
    Deserialize,
    de::{
        self, DeserializeSeed, EnumAccess, Expected, MapAccess, SeqAccess, Unexpected,
        VariantAccess, Visitor,
    },
};
use std::{fmt, marker::PhantomData, slice};

/// Deserializes a [KValue] into a type that implements [`Deserialize`]
pub fn from_koto_value<'de, T>(value: impl Into<KValue>) -> Result<T>
where
    T: Deserialize<'de>,
{
    T::deserialize(Deserializer::new(value.into())?)
}

macro_rules! deserialize_number {
    ($trait_method:ident, $type:ty, $visitor_method:ident) => {
        fn $trait_method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                KValue::Number(n) => visitor.$visitor_method(<$type>::from(n)),
                other => unsupported_error("number", &other),
            }
        }
    };
}

macro_rules! try_deserialize_number {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                KValue::Number(n) => match i64::try_from(n) {
                    Ok(i) => visitor.visit_i64(i),
                    Err(_) => Err(Error::OutOfI64RangeNumber(n)),
                },
                other => unsupported_error("number", &other),
            }
        }
    };
}

pub struct Deserializer(KValue);

impl Deserializer {
    fn new(value: KValue) -> Result<Self> {
        let value = match value {
            KValue::Object(o) => o
                .try_borrow()
                .map_err(|e| Error::FailedToSerializeKObject(e.to_string()))?
                .serialize()
                .map_err(|e| Error::FailedToSerializeKObject(e.to_string()))?,
            other => other,
        };
        Ok(Self(value))
    }
}

impl<'de> de::Deserializer<'de> for Deserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Null => visitor.visit_unit(),
            KValue::Bool(b) => visitor.visit_bool(b),
            KValue::Number(n) => match n {
                KNumber::F64(f) => visitor.visit_f64(f),
                KNumber::I64(i) => visitor.visit_i64(i),
            },
            KValue::List(l) => visit_value_slice(&l.data(), visitor),
            KValue::Tuple(t) => visit_value_slice(&t, visitor),
            KValue::Map(m) => visit_map_entries(m, visitor),
            KValue::Str(s) => visitor.visit_str(&s),
            other => unsupported_error("deserializable value", &other),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Bool(b) => visitor.visit_bool(b),
            other => unsupported_error("bool", &other),
        }
    }

    deserialize_number!(deserialize_i8, i8, visit_i8);
    deserialize_number!(deserialize_i16, i16, visit_i16);
    deserialize_number!(deserialize_i32, i32, visit_i32);
    deserialize_number!(deserialize_i64, i64, visit_i64);
    try_deserialize_number!(deserialize_i128);
    deserialize_number!(deserialize_u8, u8, visit_u8);
    deserialize_number!(deserialize_u16, u16, visit_u16);
    deserialize_number!(deserialize_u32, u32, visit_u32);
    try_deserialize_number!(deserialize_u64);
    try_deserialize_number!(deserialize_u128);
    deserialize_number!(deserialize_f32, f32, visit_f32);
    deserialize_number!(deserialize_f64, f64, visit_f64);

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Str(s) => {
                if s.chars().count() == 1 {
                    // Safe to unwrap, we just checked that there's a char in the string
                    visitor.visit_char(s.chars().next().unwrap())
                } else {
                    Err(Error::StringDoesntContainSingleChar(s.as_str().into()))
                }
            }
            other => unsupported_error("string containing a single char", &other),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Str(s) => visitor.visit_str(s.as_str()),
            other => unsupported_error("string", &other),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Str(s) => visitor.visit_string(s.as_str().to_string()),
            other => unsupported_error("string", &other),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = match self.0 {
            KValue::Str(v) => v.as_bytes().to_vec(),
            KValue::Tuple(data) => values_to_bytes(&data)?,
            KValue::List(data) => values_to_bytes(&data.data())?,
            other => return unsupported_error("string, tuple, or list", &other),
        };

        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match &self.0 {
            KValue::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Null => visitor.visit_unit(),
            other => unsupported_error("null", &other),
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Tuple(t) => visit_value_slice(&t, visitor),
            KValue::List(l) => visit_value_slice(&l.data(), visitor),
            other => unsupported_error("tuple or list", &other),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Map(m) => visit_map_entries(m, visitor),
            other => unsupported_error("map", &other),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KValue::Tuple(t) => visit_value_slice(&t, visitor),
            KValue::List(l) => visit_value_slice(&l.data(), visitor),
            KValue::Map(m) => visit_map_entries(m, visitor),
            other => unsupported_error("tuple, list, or map", &other),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match &self.0 {
            KValue::Str(_) => {
                return visitor.visit_enum(KotoEnum {
                    tag: self.0.clone(),
                    payload: KValue::Null,
                });
            }
            KValue::Map(m) => {
                let data = m.data();
                if data.len() == 1 {
                    // Safe to unwrap, we just checked that the map contains a single entry
                    let (tag, payload) = data.get_index(0).unwrap();
                    return visitor.visit_enum(KotoEnum {
                        tag: tag.value().clone(),
                        payload: payload.clone(),
                    });
                }
            }
            _ => {}
        }

        unsupported_error("string or single-entry map", &self.0)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

fn unsupported_error<T>(expected: &str, value: &KValue) -> Result<T> {
    let unexpected = match value {
        KValue::Null => Unexpected::Unit,
        KValue::Bool(b) => Unexpected::Bool(*b),
        KValue::Number(n) => match n {
            KNumber::F64(f) => Unexpected::Float(*f),
            KNumber::I64(i) => Unexpected::Signed(*i),
        },
        KValue::Range(_) => Unexpected::Other("range"),
        KValue::List(_) | KValue::Tuple(_) => Unexpected::Seq,
        KValue::Map(_) => Unexpected::Map,
        KValue::Str(s) => Unexpected::Str(s.as_str()),
        KValue::Function(_) | KValue::NativeFunction(_) => Unexpected::Other("function"),
        KValue::Iterator(_) => Unexpected::Other("iterator"),
        KValue::Object(_) => Unexpected::Other("object"),
        KValue::TemporaryTuple(_) => Unexpected::Other("temp tuple"),
    };
    Err(de::Error::invalid_type(unexpected, &expected))
}

fn values_to_bytes(values: &[KValue]) -> Result<Vec<u8>> {
    values
        .iter()
        .map(|value| match value {
            #[allow(clippy::unnecessary_fallible_conversions)]
            KValue::Number(n) => match u8::try_from(n) {
                Ok(x) => Ok(x),
                Err(_) => Err(Error::OutOfU8RangeNumber(*n)),
            },
            other => unsupported_error("number", other),
        })
        .collect::<Result<_>>()
}

fn visit_value_slice<'slice, 'de, V>(slice: &'slice [KValue], visitor: V) -> Result<V::Value>
where
    V: de::Visitor<'de>,
{
    let len = slice.len();
    let mut deserializer = ValueSliceDeserializer::new(slice);
    let seq = visitor.visit_seq(&mut deserializer)?;
    let remaining = deserializer.iter.len();
    if remaining == 0 {
        Ok(seq)
    } else {
        Err(de::Error::invalid_length(
            len,
            &ExpectedCount { count: len },
        ))
    }
}

struct ValueSliceDeserializer<'slice, 'de> {
    iter: slice::Iter<'slice, KValue>,
    _phantom: PhantomData<&'de ()>,
}

impl<'slice, 'de> ValueSliceDeserializer<'slice, 'de> {
    fn new(slice: &'slice [KValue]) -> Self {
        ValueSliceDeserializer {
            iter: slice.iter(),
            _phantom: PhantomData,
        }
    }
}

impl<'slice, 'de> SeqAccess<'de> for ValueSliceDeserializer<'slice, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed
                .deserialize(Deserializer::new(value.clone())?)
                .map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        match self.iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        }
    }
}

fn visit_map_entries<'de, V>(map: KMap, visitor: V) -> Result<V::Value>
where
    V: de::Visitor<'de>,
{
    let len = map.data().len();
    let mut deserializer = MapDeserializer::new(map);
    let seq = visitor.visit_map(&mut deserializer)?;
    if deserializer.index == len {
        Ok(seq)
    } else {
        Err(de::Error::invalid_length(
            len,
            &ExpectedCount { count: len },
        ))
    }
}

struct MapDeserializer<'de> {
    map: KMap,
    index: usize,
    value: Option<KValue>,
    _phantom: PhantomData<&'de ()>,
}

impl<'de> MapDeserializer<'de> {
    fn new(map: KMap) -> Self {
        MapDeserializer {
            map,
            index: 0,
            value: None,
            _phantom: PhantomData,
        }
    }
}

impl<'de> MapAccess<'de> for MapDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<S>(&mut self, seed: S) -> Result<Option<S::Value>>
    where
        S: DeserializeSeed<'de>,
    {
        match self.map.data().get_index(self.index) {
            Some((key, value)) => {
                self.value = Some(value.clone());
                self.index += 1;
                seed.deserialize(Deserializer::new(key.value().clone())?)
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<S>(&mut self, seed: S) -> Result<S::Value>
    where
        S: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(Deserializer::new(value)?),
            None => Err(de::Error::custom("missing value for map entry")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.map.data().len())
    }
}

struct ExpectedCount {
    count: usize,
}

impl Expected for ExpectedCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.count)
    }
}

struct KotoEnum {
    tag: KValue,
    payload: KValue,
}

impl<'de> EnumAccess<'de> for KotoEnum {
    type Error = Error;
    type Variant = EnumPayload;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((
            seed.deserialize(Deserializer::new(self.tag.clone())?)?,
            EnumPayload {
                payload: self.payload,
            },
        ))
    }
}

struct EnumPayload {
    payload: KValue,
}

impl<'de> VariantAccess<'de> for EnumPayload {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        match self.payload {
            KValue::Null => Ok(()),
            other => unsupported_error("null", &other),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(Deserializer::new(self.payload.clone())?)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(Deserializer::new(self.payload.clone())?, visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(Deserializer::new(self.payload.clone())?, visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koto_runtime::{KList, KTuple};
    use serde::Deserialize;

    #[test]
    fn to_scalar() {
        assert_eq!(Option::<()>::None, from_koto_value(KValue::Null).unwrap());
        assert_eq!(123_u8, from_koto_value(123).unwrap());
        assert!(!from_koto_value::<bool>(false).unwrap());
        assert_eq!('a', from_koto_value("a").unwrap());
        assert_eq!("xyz", from_koto_value::<String>("xyz").unwrap());
    }

    #[test]
    fn to_struct() {
        #[derive(Deserialize)]
        struct TestStruct {
            foo: i64,
            bar: Vec<String>,
        }

        let map = KMap::default();
        map.insert("foo", -1);
        map.insert(
            "bar",
            KTuple::from(vec!["abc".into(), "def".into(), "ghi".into()]),
        );

        let result: TestStruct = from_koto_value(map).unwrap();

        assert_eq!(result.foo, -1);
        assert_eq!(&result.bar, &["abc", "def", "ghi"]);
    }

    #[test]
    fn to_enum() {
        #[derive(Deserialize, Debug, PartialEq)]
        enum TestEnum {
            A,
            B,
            C(String),
            D(i64, bool),
            E {
                foo: (Option<u8>, f32),
                bar: Vec<char>,
            },
        }

        assert_eq!(TestEnum::A, from_koto_value("A").unwrap());
        assert_eq!(TestEnum::B, from_koto_value("B").unwrap());

        let enum_c = KMap::new();
        enum_c.insert("C", "abc");
        assert_eq!(TestEnum::C("abc".into()), from_koto_value(enum_c).unwrap());

        let enum_d = KMap::new();
        enum_d.insert("D", KValue::Tuple(vec![42.into(), false.into()].into()));
        assert_eq!(TestEnum::D(42, false), from_koto_value(enum_d).unwrap());

        let enum_e = KMap::new();
        let enum_e_fields = KMap::new();
        enum_e_fields.insert("foo", KValue::Tuple(vec![99.into(), 1.0.into()].into()));
        enum_e_fields.insert(
            "bar",
            KValue::List(KList::from_slice(&["x".into(), "y".into(), "z".into()])),
        );
        enum_e.insert("E", enum_e_fields);
        assert_eq!(
            TestEnum::E {
                foo: (Some(99), 1.0),
                bar: vec!['x', 'y', 'z'],
            },
            from_koto_value(enum_e).unwrap()
        );
    }
}
