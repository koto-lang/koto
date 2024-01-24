//! The core value type used in the Koto runtime

use crate::{prelude::*, KCaptureFunction, KFunction, KMap, KNativeFunction, Result};
use std::fmt::{self, Write};

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

    /// A Koto function with captures
    CaptureFunction(Ptr<KCaptureFunction>),

    /// A function that's implemented outside of the Koto runtime
    NativeFunction(KNativeFunction),

    /// The iterator type used in Koto
    Iterator(KIterator),

    /// An object with behaviour defined via the [KotoObject] trait
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
    /// This is used by koto.deep_copy.
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
            Function(f) if f.generator => false,
            CaptureFunction(f) if f.info.generator => false,
            Function(_) | CaptureFunction(_) | NativeFunction(_) => true,
            Map(m) => m.contains_meta_key(&MetaKey::Call),
            _ => false,
        }
    }

    /// Returns true if the value is a generator function
    pub fn is_generator(&self) -> bool {
        use KValue::*;
        match self {
            Function(f) if f.generator => true,
            CaptureFunction(f) if f.info.generator => true,
            _ => false,
        }
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

    /// Returns true if a [KIterator] can be made from the value
    pub fn is_iterable(&self) -> bool {
        use KValue::*;
        match self {
            Range(_) | List(_) | Tuple(_) | Map(_) | Str(_) | Iterator(_) => true,
            Object(o) => o.try_borrow().map_or(false, |o| {
                !matches!(o.is_iterable(), IsIterable::NotIterable)
            }),
            _ => false,
        }
    }

    /// Returns the 'size' of the value
    ///
    /// A value's size is the number of elements that can used in unpacking expressions
    /// e.g.
    /// x = [1, 2, 3] # x has size 3
    /// a, b, c = x
    ///
    /// See:
    ///   - [Op::Size](koto_bytecode::Op::Size)
    ///   - [Op::CheckSizeEqual](koto_bytecode::Op::CheckSizeEqual).
    ///   - [Op::CheckSizeMin](koto_bytecode::Op::CheckSizeMin).
    pub fn size(&self) -> usize {
        use KValue::*;

        match &self {
            List(l) => l.len(),
            Tuple(t) => t.len(),
            TemporaryTuple(RegisterSlice { count, .. }) => *count as usize,
            Map(m) => m.len(),
            _ => 1,
        }
    }

    /// Returns the value's type as a [KString]
    pub fn type_as_string(&self) -> KString {
        use KValue::*;
        match &self {
            Null => TYPE_NULL.with(|x| x.clone()),
            Bool(_) => TYPE_BOOL.with(|x| x.clone()),
            Number(KNumber::F64(_)) => TYPE_FLOAT.with(|x| x.clone()),
            Number(KNumber::I64(_)) => TYPE_INT.with(|x| x.clone()),
            List(_) => TYPE_LIST.with(|x| x.clone()),
            Range { .. } => TYPE_RANGE.with(|x| x.clone()),
            Map(m) if m.meta_map().is_some() => match m.get_meta_value(&MetaKey::Type) {
                Some(Str(s)) => s,
                Some(_) => "Error: expected string for overloaded type".into(),
                None => TYPE_OBJECT.with(|x| x.clone()),
            },
            Map(_) => TYPE_MAP.with(|x| x.clone()),
            Str(_) => TYPE_STRING.with(|x| x.clone()),
            Tuple(_) => TYPE_TUPLE.with(|x| x.clone()),
            Function(f) if f.generator => TYPE_GENERATOR.with(|x| x.clone()),
            CaptureFunction(f) if f.info.generator => TYPE_GENERATOR.with(|x| x.clone()),
            Function(_) | CaptureFunction(_) | NativeFunction(_) => {
                TYPE_FUNCTION.with(|x| x.clone())
            }
            Object(o) => o.try_borrow().map_or_else(
                |_| "Error: object already borrowed".into(),
                |o| o.type_string(),
            ),
            Iterator(_) => TYPE_ITERATOR.with(|x| x.clone()),
            TemporaryTuple { .. } => TYPE_TEMPORARY_TUPLE.with(|x| x.clone()),
        }
    }

    /// Returns true if the value is a Map or an External that contains the given meta key
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        use KValue::*;
        match &self {
            Map(m) => m.contains_meta_key(key),
            _ => false,
        }
    }

    /// If the value is a Map or an External, returns a clone of the corresponding meta value
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<KValue> {
        use KValue::*;
        match &self {
            Map(m) => m.get_meta_value(key),
            _ => None,
        }
    }

    /// Renders the value into the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        use KValue::*;
        let result = match self {
            Null => write!(ctx, "null"),
            Bool(b) => write!(ctx, "{b}"),
            Number(n) => write!(ctx, "{n}"),
            Range(r) => write!(ctx, "{r}"),
            Function(_) | CaptureFunction(_) => write!(ctx, "||"),
            Iterator(_) => write!(ctx, "Iterator"),
            NativeFunction(_) => write!(ctx, "||"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(ctx, "TemporaryTuple [{start}..{}]", start + count)
            }
            Str(s) => return s.display(ctx),
            List(l) => return l.display(ctx),
            Tuple(t) => return t.display(ctx),
            Map(m) => return m.display(ctx),
            Object(o) => return o.try_borrow()?.display(ctx),
        };
        if result.is_ok() {
            Ok(())
        } else {
            runtime_error!("Failed to write to string")
        }
    }
}

thread_local! {
    static TYPE_NULL: KString = "Null".into();
    static TYPE_BOOL: KString = "Bool".into();
    static TYPE_FLOAT: KString = "Float".into();
    static TYPE_INT: KString = "Int".into();
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

/// A slice of a VM's registers
///
/// See [Value::TemporaryTuple]
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterSlice {
    pub start: u8,
    pub count: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_mem_size() {
        // All Value variants should have a size of <= 16 bytes, and with the variant flag the
        // total size of Value will be <= 24 bytes.
        assert!(std::mem::size_of::<KValue>() <= 24);
    }
}
