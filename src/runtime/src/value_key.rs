use {
    crate::prelude::*,
    std::{
        borrow::Borrow,
        cmp::Ordering,
        hash::{Hash, Hasher},
        ops::Deref,
    },
};

/// The key type used by [ValueMap](crate::ValueMap)
///
/// Only immutable values can be used as keys, see [Value::is_hashable]
#[derive(Clone, Debug)]
pub struct ValueKey(Value);

impl ValueKey {
    /// Returns a reference to the key's value
    pub fn value(&self) -> &Value {
        &self.0
    }

    /// Returns a display string for the key
    pub fn key_to_string(&self) -> Result<String, RuntimeError> {
        use Value::*;

        let result = match &self.0 {
            Null => "null".to_string(),
            Bool(b) => b.to_string(),
            Number(n) => n.to_string(),
            Range(r) => r.to_string(),
            Str(s) => s.to_string(),
            unexpected => {
                return runtime_error!("Invalid ValueKeyType: {}", unexpected.type_as_string())
            }
        };

        Ok(result)
    }
}

impl PartialEq for ValueKey {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (&self.0, &other.0) {
            (Number(a), Number(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (Null, Null) => true,
            _ => false,
        }
    }
}

impl TryFrom<Value> for ValueKey {
    type Error = RuntimeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value.is_hashable() {
            Ok(Self(value))
        } else {
            runtime_error!("Only hashable values can be used as value keys")
        }
    }
}

impl From<ValueString> for ValueKey {
    fn from(value: ValueString) -> Self {
        Self(Value::Str(value))
    }
}

impl From<&str> for ValueKey {
    fn from(value: &str) -> Self {
        Self(Value::Str(value.into()))
    }
}

impl Borrow<Value> for ValueKey {
    fn borrow(&self) -> &Value {
        &self.0
    }
}

impl Deref for ValueKey {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for ValueKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl PartialOrd for ValueKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Value::*;

        match (&self.0, &other.0) {
            (Null, Null) => Some(Ordering::Equal),
            (Null, _) => Some(Ordering::Less),
            (_, Null) => Some(Ordering::Greater),
            (Number(a), Number(b)) => a.partial_cmp(b),
            (Str(a), Str(b)) => a.partial_cmp(b),
            _ => Some(Ordering::Less),
        }
    }
}

impl Ord for ValueKey {
    fn cmp(&self, other: &Self) -> Ordering {
        use Value::*;

        match (&self.0, &other.0) {
            (Null, Null) => Ordering::Equal,
            (Null, _) => Ordering::Less,
            (_, Null) => Ordering::Greater,
            (Number(a), Number(b)) => a.cmp(b),
            (Str(a), Str(b)) => a.cmp(b),
            _ => Ordering::Less,
        }
    }
}

impl Eq for ValueKey {}

// Currently only used to support DataMap::get_with_string()
#[derive(Clone, Debug)]
pub(crate) enum ValueRef<'a> {
    Null,
    Bool(&'a bool),
    Number(&'a ValueNumber),
    Str(&'a str),
    Range(&'a IntRange),
}

impl<'a> From<&'a Value> for ValueRef<'a> {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Null => ValueRef::Null,
            Value::Bool(b) => ValueRef::Bool(b),
            Value::Number(n) => ValueRef::Number(n),
            Value::Str(s) => ValueRef::Str(s),
            Value::Range(r) => ValueRef::Range(r),
            _ => unreachable!(), // Only immutable values can be used in ValueKey
        }
    }
}

impl<'a> PartialEq for ValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        use ValueRef::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (Null, Null) => true,
            _ => false,
        }
    }
}

impl<'a> Eq for ValueRef<'a> {}

impl<'a> Hash for ValueRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use ValueRef::*;

        std::mem::discriminant(self).hash(state);

        match self {
            Null => {}
            Bool(b) => b.hash(state),
            Number(n) => n.hash(state),
            Str(s) => s.hash(state),
            Range(IntRange { start, end }) => {
                state.write_isize(*start);
                state.write_isize(*end);
            }
        }
    }
}

// A trait that allows for allocation-free map accesses with &str
pub(crate) trait ValueKeyRef {
    fn to_value_ref(&self) -> ValueRef;
}

impl<'a> Hash for dyn ValueKeyRef + 'a {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_value_ref().hash(state);
    }
}

impl<'a> PartialEq for dyn ValueKeyRef + 'a {
    fn eq(&self, other: &Self) -> bool {
        self.to_value_ref() == other.to_value_ref()
    }
}

impl<'a> Eq for dyn ValueKeyRef + 'a {}

impl ValueKeyRef for Value {
    fn to_value_ref(&self) -> ValueRef {
        self.as_ref()
    }
}

impl ValueKeyRef for ValueKey {
    fn to_value_ref(&self) -> ValueRef {
        self.0.as_ref()
    }
}

// The key part of this whole mechanism; wrap a &str as ValueRef::Str,
// allowing a map search to be performed directly against &str
impl<'a> ValueKeyRef for &'a str {
    fn to_value_ref(&self) -> ValueRef {
        ValueRef::Str(self)
    }
}

impl<'a> Borrow<dyn ValueKeyRef + 'a> for ValueKey {
    fn borrow(&self) -> &(dyn ValueKeyRef + 'a) {
        self
    }
}

impl<'a> Borrow<dyn ValueKeyRef + 'a> for &'a str {
    fn borrow(&self) -> &(dyn ValueKeyRef + 'a) {
        self
    }
}
