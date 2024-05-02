use crate::{prelude::*, Error};
use indexmap::Equivalent;
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

/// The key type used by [ValueMap](crate::ValueMap)
///
/// Only hashable values can be used as keys, see [KValue::is_hashable]
#[derive(Clone)]
pub struct ValueKey(KValue);

impl ValueKey {
    /// Returns a reference to the key's value
    pub fn value(&self) -> &KValue {
        &self.0
    }
}

impl TryFrom<KValue> for ValueKey {
    type Error = Error;

    fn try_from(value: KValue) -> Result<Self, Self::Error> {
        if value.is_hashable() {
            Ok(Self(value))
        } else {
            runtime_error!("Only hashable values can be used as value keys")
        }
    }
}

impl PartialEq for ValueKey {
    fn eq(&self, other: &Self) -> bool {
        use KValue::*;

        match (&self.0, &other.0) {
            (Number(a), Number(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (Null, Null) => true,
            (Tuple(a), Tuple(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(value_a, value_b)| Self(value_a.clone()) == Self(value_b.clone()))
            }
            _ => false,
        }
    }
}
impl Eq for ValueKey {}

impl Hash for ValueKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use KValue::*;

        match &self.0 {
            Null => {}
            Bool(b) => b.hash(state),
            Number(n) => n.hash(state),
            Str(s) => s.hash(state),
            Range(r) => r.hash(state),
            Tuple(t) => {
                for value in t.iter() {
                    Self(value.clone()).hash(state)
                }
            }
            _ => {}
        }
    }
}

impl PartialOrd for ValueKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use KValue::*;

        match (&self.0, &other.0) {
            (Null, Null) => Some(Ordering::Equal),
            (Null, _) => Some(Ordering::Less),
            (_, Null) => Some(Ordering::Greater),
            (Number(a), Number(b)) => a.partial_cmp(b),
            (Str(a), Str(b)) => a.partial_cmp(b),
            (Tuple(a), Tuple(b)) => match a.len().cmp(&b.len()) {
                Ordering::Equal => {
                    for (value_a, value_b) in a.iter().zip(b.iter()) {
                        // Only ValueRef-able values will be contained in a tuple that's made it
                        // into a ValueKey
                        match Self(value_a.clone()).partial_cmp(&Self(value_b.clone())) {
                            Some(Ordering::Equal) => {}
                            other => return other,
                        }
                    }
                    Some(Ordering::Equal)
                }
                other => Some(other),
            },
            _ => Some(Ordering::Equal),
        }
    }
}

impl fmt::Display for ValueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        use KValue::*;

        match &self.0 {
            Null => f.write_str("null"),
            Bool(b) => write!(f, "{b}"),
            Number(n) => write!(f, "{n}"),
            Range(r) => write!(f, "{r}"),
            Str(s) => f.write_str(s),
            Tuple(t) => {
                f.write_str("(")?;
                for (i, value) in t.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}", Self(value.clone()))?;
                }
                f.write_str(")")
            }
            _ => Ok(()),
        }
    }
}

impl From<KString> for ValueKey {
    fn from(value: KString) -> Self {
        Self(KValue::Str(value))
    }
}

impl<T> From<T> for ValueKey
where
    KNumber: From<T>,
{
    fn from(value: T) -> Self {
        Self(KValue::Number(value.into()))
    }
}

impl From<&str> for ValueKey {
    fn from(value: &str) -> Self {
        Self(KValue::Str(value.into()))
    }
}

// Support efficient map accesses with &str
impl Equivalent<ValueKey> for str {
    fn equivalent(&self, other: &ValueKey) -> bool {
        match &other.0 {
            KValue::Str(s) => self == s.as_str(),
            _ => false,
        }
    }
}

impl Equivalent<ValueKey> for KString {
    fn equivalent(&self, other: &ValueKey) -> bool {
        match &other.0 {
            KValue::Str(s) => self == s,
            _ => false,
        }
    }
}
