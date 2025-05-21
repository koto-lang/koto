use koto_runtime::{KValue, ValueKey, ValueMap};
use serde::{
    Deserialize, Deserializer,
    de::{self, Unexpected, VariantAccess, Visitor},
};
use std::{
    fmt,
    ops::{Deref, DerefMut},
};

/// A newtype for [`KValue`] that implements [`serde::Deserialize`]
#[derive(Clone, Default)]
pub struct DeserializableKValue(pub KValue);

impl From<DeserializableKValue> for KValue {
    fn from(value: DeserializableKValue) -> Self {
        value.0
    }
}

impl From<KValue> for DeserializableKValue {
    fn from(value: KValue) -> Self {
        Self(value)
    }
}

impl Deref for DeserializableKValue {
    type Target = KValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DeserializableKValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<KValue> for DeserializableKValue {
    fn as_ref(&self) -> &KValue {
        self.deref()
    }
}

impl AsMut<KValue> for DeserializableKValue {
    fn as_mut(&mut self) -> &mut KValue {
        self.deref_mut()
    }
}

struct KValueVisitor;

impl<'de> Visitor<'de> for KValueVisitor {
    type Value = DeserializableKValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a deserializable value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(v.into()))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(v.into()))
    }

    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        i64::try_from(v)
            .map(|v| DeserializableKValue(v.into()))
            .map_err(|_| {
                de::Error::invalid_value(Unexpected::Other("i128"), &"integer in i64 range")
            })
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        i64::try_from(v)
            .map(|v| DeserializableKValue(v.into()))
            .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(v), &"integer in i64 range"))
    }

    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        i64::try_from(v)
            .map(|v| DeserializableKValue(v.into()))
            .map_err(|_| {
                de::Error::invalid_value(Unexpected::Other("u128"), &"integer in i64 range")
            })
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(v.into()))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(v.into()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let data = v
            .iter()
            .map(|x| KValue::Number(x.into()))
            .collect::<Vec<_>>();
        Ok(KValue::Tuple(data.into()).into())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(KValue::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(DeserializableKValue(KValue::Null))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut data = Vec::with_capacity(seq.size_hint().unwrap_or_default());

        while let Some(next) = seq.next_element::<DeserializableKValue>()? {
            data.push(next.into());
        }

        Ok(DeserializableKValue(KValue::Tuple(data.into())))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut data = ValueMap::with_capacity(map.size_hint().unwrap_or_default());

        while let Some((key, value)) =
            map.next_entry::<DeserializableKValue, DeserializableKValue>()?
        {
            let key = ValueKey::try_from(KValue::from(key)).map_err(|_| {
                de::Error::invalid_value(
                    Unexpected::Other("a value that can be used as a map key"),
                    &"a value that can't be used as a map key",
                )
            })?;
            data.insert(key, value.into());
        }

        Ok(DeserializableKValue(KValue::Map(data.into())))
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: de::EnumAccess<'de>,
    {
        let (variant, variant_access) = data.variant::<DeserializableKValue>()?;

        let key = ValueKey::try_from(KValue::from(variant)).map_err(|_| {
            de::Error::invalid_value(
                Unexpected::Other("a value that can be used as a map key"),
                &"a value that can't be used as a map key",
            )
        })?;
        let value = variant_access.newtype_variant::<DeserializableKValue>()?;

        let mut data = ValueMap::default();
        data.insert(key, value.into());
        Ok(KValue::Map(data.into()).into())
    }
}

impl<'de> Deserialize<'de> for DeserializableKValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(KValueVisitor)
    }
}
