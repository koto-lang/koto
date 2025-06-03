//! The core value type used in the Koto runtime

use crate::{KFunction, Ptr, Result, prelude::*};
use std::{
    fmt::{self, Write},
    result::Result as StdResult,
};

/// The core Value type for Koto
#[derive(Clone, Default)]
pub enum KValue {
    /// The default type representing the absence of a value
    #[default]
    Null,

    /// A boolean, can be either true or false
    Bool(bool),

    /// A number, represented as either a signed 64 bit integer or float
    Number(KNumber),

    /// A range with start/end boundaries
    Range(KRange),

    /// The list type used in Koto
    List(KList),

    /// The tuple type used in Koto
    Tuple(KTuple),

    /// The hash map type used in Koto
    Map(KMap),

    /// The string type used in Koto
    Str(KString),

    /// A Koto function
    Function(KFunction),

    /// A function that's implemented outside of the Koto runtime
    NativeFunction(KNativeFunction),

    /// The iterator type used in Koto
    Iterator(KIterator),

    /// An object with behaviour defined via the [`KotoObject`] trait
    Object(KObject),

    /// A tuple of values that are packed into a contiguous series of registers
    ///
    /// Used as an optimization when multiple values are passed around without being assigned to a
    /// single Tuple value.
    ///
    /// Note: this is intended for internal use only.
    TemporaryTuple(RegisterSlice),
}

impl KValue {
    /// Returns a recursive 'deep copy' of a Value
    ///
    /// This is used by `koto.deep_copy`.
    pub fn deep_copy(&self) -> Result<KValue> {
        let result = match &self {
            KValue::List(l) => {
                let result = l
                    .data()
                    .iter()
                    .map(|v| v.deep_copy())
                    .collect::<Result<_>>()?;
                KList::with_data(result).into()
            }
            KValue::Tuple(t) => {
                let result = t
                    .iter()
                    .map(|v| v.deep_copy())
                    .collect::<Result<Vec<_>>>()?;
                KValue::Tuple(result.into())
            }
            KValue::Map(m) => {
                let data = m
                    .data()
                    .iter()
                    .map(|(k, v)| v.deep_copy().map(|v| (k.clone(), v)))
                    .collect::<Result<_>>()?;
                let meta = m.meta_map().map(|meta| meta.borrow().clone());
                KMap::with_contents(data, meta).into()
            }
            KValue::Iterator(i) => i.make_copy()?.into(),
            KValue::Object(o) => o.try_borrow()?.copy().into(),
            _ => self.clone(),
        };

        Ok(result)
    }

    /// Returns true if the value has function-like callable behaviour
    pub fn is_callable(&self) -> bool {
        use KValue::*;
        match self {
            Function(f) if f.flags.is_generator() => false,
            Function(_) | NativeFunction(_) => true,
            Map(m) => m.contains_meta_key(&MetaKey::Call),
            Object(o) => o.try_borrow().is_ok_and(|o| o.is_callable()),
            _ => false,
        }
    }

    /// Returns true if the value is a generator function
    pub fn is_generator(&self) -> bool {
        matches!(self, KValue::Function(f) if f.flags.is_generator())
    }

    /// Returns true if the value is hashable
    ///
    /// Only hashable values are acceptable as map keys.
    pub fn is_hashable(&self) -> bool {
        use KValue::*;
        match self {
            Null | Bool(_) | Number(_) | Range(_) | Str(_) => true,
            Tuple(t) => t.is_hashable(),
            _ => false,
        }
    }

    /// Returns true if the value supports `[]` indexing operations
    pub fn is_indexable(&self) -> bool {
        use KValue::*;
        match self {
            List(_) | Map(_) | Str(_) | Tuple(_) => true,
            Object(o) => o.try_borrow().is_ok_and(|o| o.size().is_some()),
            _ => false,
        }
    }

    /// Returns true if a [KIterator] can be made from the value
    pub fn is_iterable(&self) -> bool {
        use KValue::*;
        match self {
            Range(_) | List(_) | Tuple(_) | Str(_) | Iterator(_) => true,
            Map(m) => {
                if m.meta_map().is_some() {
                    m.contains_meta_key(&UnaryOp::Iterator.into())
                        || m.contains_meta_key(&UnaryOp::Next.into())
                } else {
                    true
                }
            }
            Object(o) => o
                .try_borrow()
                .is_ok_and(|o| !matches!(o.is_iterable(), IsIterable::NotIterable)),
            _ => false,
        }
    }

    /// Returns the value's type as a [KString]
    pub fn type_as_string(&self) -> KString {
        use KValue::*;
        match &self {
            Null => TYPE_NULL.with(|x| x.clone()),
            Bool(_) => TYPE_BOOL.with(|x| x.clone()),
            Number(_) => TYPE_NUMBER.with(|x| x.clone()),
            List(_) => TYPE_LIST.with(|x| x.clone()),
            Range { .. } => TYPE_RANGE.with(|x| x.clone()),
            Map(m) if m.meta_map().is_some() => match m.get_meta_value(&MetaKey::Type) {
                Some(Str(s)) => s,
                Some(_) => "Error: expected string as result of @type".into(),
                None => match m.get_meta_value(&MetaKey::Base) {
                    Some(base @ Map(_)) => base.type_as_string(),
                    _ => TYPE_OBJECT.with(|x| x.clone()),
                },
            },
            Map(_) => TYPE_MAP.with(|x| x.clone()),
            Str(_) => TYPE_STRING.with(|x| x.clone()),
            Tuple(_) => TYPE_TUPLE.with(|x| x.clone()),
            Function(f) if f.flags.is_generator() => TYPE_GENERATOR.with(|x| x.clone()),
            Function(_) | NativeFunction(_) => TYPE_FUNCTION.with(|x| x.clone()),
            Object(o) => o.try_borrow().map_or_else(
                |_| "Error: object already borrowed".into(),
                |o| o.type_string(),
            ),
            Iterator(_) => TYPE_ITERATOR.with(|x| x.clone()),
            TemporaryTuple { .. } => TYPE_TEMPORARY_TUPLE.with(|x| x.clone()),
        }
    }

    /// Renders the value into the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        use KValue::*;
        let _ = match self {
            Null => write!(ctx, "null"),
            Bool(b) => write!(ctx, "{b}"),
            Number(n) => write!(ctx, "{n}"),
            Range(r) => write!(ctx, "{r}"),
            Function(f) => {
                if ctx.debug_enabled() {
                    write!(ctx, "|| (chunk: {}, ip: {})", Ptr::address(&f.chunk), f.ip)
                } else {
                    write!(ctx, "||")
                }
            }
            NativeFunction(f) => {
                if ctx.debug_enabled() {
                    write!(ctx, "|| ({})", Ptr::address(&f.function))
                } else {
                    write!(ctx, "||")
                }
            }
            Iterator(_) => write!(ctx, "Iterator"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(ctx, "TemporaryTuple [{start}..{}]", start + count)
            }
            Str(s) => {
                if ctx.is_contained() || ctx.debug_enabled() {
                    write!(ctx, "\'{s}\'")
                } else {
                    write!(ctx, "{s}")
                }
            }
            List(l) => return l.display(ctx),
            Tuple(t) => return t.display(ctx),
            Map(m) => return m.display(ctx),
            Object(o) => return o.try_borrow()?.display(ctx),
        };
        Ok(())
    }
}

thread_local! {
    static TYPE_NULL: KString = "Null".into();
    static TYPE_BOOL: KString = "Bool".into();
    static TYPE_NUMBER: KString = "Number".into();
    static TYPE_LIST: KString = "List".into();
    static TYPE_RANGE: KString = "Range".into();
    static TYPE_MAP: KString = "Map".into();
    static TYPE_OBJECT: KString = "Object".into();
    static TYPE_STRING: KString = "String".into();
    static TYPE_TUPLE: KString = "Tuple".into();
    static TYPE_FUNCTION: KString = "Function".into();
    static TYPE_GENERATOR: KString = "Generator".into();
    static TYPE_ITERATOR: KString = "Iterator".into();
    static TYPE_TEMPORARY_TUPLE: KString = "TemporaryTuple".into();
}

impl fmt::Debug for KValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_as_string())
    }
}

impl From<()> for KValue {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<bool> for KValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<KNumber> for KValue {
    fn from(value: KNumber) -> Self {
        Self::Number(value)
    }
}

impl From<KRange> for KValue {
    fn from(value: KRange) -> Self {
        Self::Range(value)
    }
}

impl From<&str> for KValue {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<String> for KValue {
    fn from(value: String) -> Self {
        Self::Str(value.into())
    }
}

impl From<KString> for KValue {
    fn from(value: KString) -> Self {
        Self::Str(value)
    }
}

impl From<KList> for KValue {
    fn from(value: KList) -> Self {
        Self::List(value)
    }
}

impl From<KTuple> for KValue {
    fn from(value: KTuple) -> Self {
        Self::Tuple(value)
    }
}

impl From<KMap> for KValue {
    fn from(value: KMap) -> Self {
        Self::Map(value)
    }
}

impl From<KObject> for KValue {
    fn from(value: KObject) -> Self {
        Self::Object(value)
    }
}

impl From<KIterator> for KValue {
    fn from(value: KIterator) -> Self {
        Self::Iterator(value)
    }
}

impl From<KNativeFunction> for KValue {
    fn from(value: KNativeFunction) -> Self {
        Self::NativeFunction(value)
    }
}

impl<T> From<T> for KValue
where
    T: KotoFunction,
{
    fn from(value: T) -> Self {
        Self::NativeFunction(KNativeFunction::new(value))
    }
}

impl<T> From<Option<T>> for KValue
where
    T: Into<KValue>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => KValue::Null,
        }
    }
}

/// A slice of a VM's register stack
///
/// See [Value::TemporaryTuple]
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterSlice {
    pub start: u8,
    pub count: u8,
}

/// If conversion fails then the input value will be returned.
impl TryFrom<KValue> for bool {
    type Error = KValue;

    fn try_from(value: KValue) -> StdResult<Self, KValue> {
        if let KValue::Bool(b) = value {
            Ok(b)
        } else {
            Err(value)
        }
    }
}

macro_rules! impl_try_from_value_string {
    ($($type:ty),+) => {
        $(
            /// If conversion fails then the input value will be returned.
            impl TryFrom<KValue> for $type {
                type Error = KValue;

                fn try_from(value: KValue) -> StdResult<Self, KValue> {
                    if let KValue::Str(s) = value {
                        Ok(s.as_str().into())
                    } else {
                        Err(value)
                    }
                }
            }
        )+
    };
}

macro_rules! impl_try_from_value_string_ref {
    ($($type:ty),+) => {
        $(
            /// If conversion fails then the input value will be returned.
            impl<'a> TryFrom<&'a KValue> for $type {
                type Error = &'a KValue;

                fn try_from(value: &'a KValue) -> StdResult<Self, &'a KValue> {
                    if let KValue::Str(s) = value {
                        Ok(s.as_str().into())
                    } else {
                        Err(value)
                    }
                }
            }
        )+
    };
}

macro_rules! impl_try_from_value_number {
    ($($type:ty),+) => {
        $(
            /// If conversion fails then the input value will be returned.
            ///
            /// Note that number conversions are lossy. Out of range values will be saturated to the
            /// bounds of the output type. Conversions follow the rules of the `as` operator.
            impl TryFrom<KValue> for $type {
                type Error = KValue;

                fn try_from(value: KValue) -> StdResult<Self, KValue> {
                    if let KValue::Number(n) = value {
                        Ok(n.into())
                    } else {
                        Err(value)
                    }
                }
            }
        )+
    };
}

impl_try_from_value_string!(String, Box<str>, std::rc::Rc<str>, std::sync::Arc<str>);
impl_try_from_value_string_ref!(&'a str, std::borrow::Cow<'a, str>);
impl_try_from_value_number!(
    f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_mem_size() {
        // All KValue variants except for KValue::Function` should have a size of <= 16 bytes.
        // KFunction has a size of 24 bytes, but is the single variant of that size,
        // and has a niche which is then usable as the niche for KValue.
        assert!(size_of::<KString>() <= 16);
        assert!(size_of::<KList>() <= 16);
        assert!(size_of::<KMap>() <= 16);
        assert!(size_of::<KObject>() <= 16);
        assert!(size_of::<KFunction>() <= 24);
        assert!(size_of::<KValue>() <= 24);
    }

    #[test]
    fn try_from_kvalue() {
        assert_eq!(
            &String::try_from(KValue::from("testing")).unwrap(),
            "testing"
        );

        assert_eq!(i32::try_from(KValue::from(-123.45)).unwrap(), -123);

        assert!(matches!(bool::try_from(KValue::Null), Err(KValue::Null)));
    }
}
