use {
    crate::{
        num2, num4, value_key::ValueRef, value_map::ValueMap, ExternalData, ExternalFunction,
        ExternalValue, IntRange, MetaKey, RwLock, ValueIterator, ValueList, ValueNumber,
        ValueString, ValueTuple, ValueVec,
    },
    koto_bytecode::Chunk,
    std::{fmt, sync::Arc},
};

/// The core Value type for Koto
#[derive(Clone, Debug)]
pub enum Value {
    /// The default type representing the absence of a value
    Empty,

    /// A boolean, can be either true or false
    Bool(bool),

    /// A number, represented as either a signed 64 bit integer or float
    Number(ValueNumber),

    /// A pair of 64 bit floats, useful when working with 2 dimensional values
    Num2(num2::Num2),

    /// A pack of four 32 bit floats, useful in working with 3 or 4 dimensional values
    Num4(num4::Num4),

    /// A range with start/end boundaries
    Range(IntRange),

    /// The list type used in Koto
    List(ValueList),

    /// The tuple type used in Koto
    Tuple(ValueTuple),

    /// The hash map type used in Koto
    Map(ValueMap),

    /// The string type used in Koto
    Str(ValueString),

    /// A callable function
    Function(RuntimeFunction),

    /// A function that produces an Iterator when called
    ///
    /// A [Vm] gets spawned for the function to run in, which pauses each time a yield instruction
    /// is encountered. See Vm::call_generator and Iterable::Generator.
    Generator(RuntimeFunction),

    /// The iterator type used in Koto
    Iterator(ValueIterator),

    /// A function that's defined outside of the Koto runtime
    ExternalFunction(ExternalFunction),

    /// A value type that's defined outside of the Koto runtime
    ExternalValue(ExternalValue),

    /// A 'data-only' counterpart to ExternalValue
    ExternalData(Arc<RwLock<dyn ExternalData>>),

    /// The range type used as a temporary value in index expressions.
    ///
    /// Note: this is intended for internal use only.
    IndexRange(IndexRange),

    /// A tuple of values that are packed into a contiguous series of registers
    ///
    /// Used as an optimization when multiple values are passed around without being assigned to a
    /// single Tuple value.
    ///
    /// Note: this is intended for internal use only.
    TemporaryTuple(RegisterSlice),
}

impl Value {
    #[inline]
    pub(crate) fn as_ref(&self) -> ValueRef {
        match &self {
            Value::Empty => ValueRef::Empty,
            Value::Bool(b) => ValueRef::Bool(b),
            Value::Number(n) => ValueRef::Number(n),
            Value::Num2(n) => ValueRef::Num2(n),
            Value::Num4(n) => ValueRef::Num4(n),
            Value::Str(s) => ValueRef::Str(&s),
            Value::Range(r) => ValueRef::Range(r),
            _ => unreachable!(), // Only immutable values can be used in ValueKey
        }
    }

    pub fn deep_copy(&self) -> Value {
        use Value::{List, Map, Tuple};

        match &self {
            List(l) => {
                let result = l.data().iter().map(|v| v.deep_copy()).collect::<ValueVec>();
                List(ValueList::with_data(result))
            }
            Tuple(t) => {
                let result = t.data().iter().map(|v| v.deep_copy()).collect::<Vec<_>>();
                Tuple(result.into())
            }
            Map(m) => {
                let data = m
                    .data()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.deep_copy()))
                    .collect();
                let meta = m.meta().clone();
                Map(ValueMap::with_contents(data, meta))
            }
            _ => self.clone(),
        }
    }

    pub fn is_callable(&self) -> bool {
        matches!(self, Value::Function { .. } | Value::ExternalFunction(_))
    }

    pub fn is_immutable(&self) -> bool {
        use Value::*;
        matches!(
            self,
            Empty | Bool(_) | Number(_) | Num2(_) | Num4(_) | Range(_) | Str(_)
        )
    }

    pub fn is_iterable(&self) -> bool {
        use Value::*;
        matches!(
            self,
            Range(_) | List(_) | Tuple(_) | Map(_) | Str(_) | Iterator(_)
        )
    }

    /// Returns the 'size' of the value
    ///
    /// A value's size is the number of elements that can used in unpacking expressions
    /// e.g.
    /// x = [1, 2, 3] # x has size 3
    /// a, b, c = x
    ///
    /// See [Op::Size] and [Op::CheckSize]
    pub fn size(&self) -> usize {
        use Value::*;

        match &self {
            List(l) => l.len(),
            Tuple(t) => t.data().len(),
            TemporaryTuple(RegisterSlice { count, .. }) => *count as usize,
            Map(m) => m.len(),
            Num2(_) => 2,
            Num4(_) => 4,
            _ => 1,
        }
    }

    pub fn type_as_string(&self) -> String {
        use Value::*;
        match &self {
            Empty => "Empty".to_string(),
            Bool(_) => "Bool".to_string(),
            Number(ValueNumber::F64(_)) => "Float".to_string(),
            Number(ValueNumber::I64(_)) => "Int".to_string(),
            Num2(_) => "Num2".to_string(),
            Num4(_) => "Num4".to_string(),
            List(_) => "List".to_string(),
            Range { .. } => "Range".to_string(),
            IndexRange { .. } => "IndexRange".to_string(),
            Map(m) => match m.meta().get(&MetaKey::Type) {
                Some(Str(s)) => s.as_str().to_string(),
                Some(_) => "Error: expected string for overloaded type".to_string(),
                None => "Map".to_string(),
            },
            Str(_) => "String".to_string(),
            Tuple(_) => "Tuple".to_string(),
            Function { .. } => "Function".to_string(),
            Generator { .. } => "Generator".to_string(),
            ExternalFunction(_) => "ExternalFunction".to_string(),
            ExternalValue(value) => match value.meta().get(&MetaKey::Type) {
                Some(Str(s)) => s.as_str().to_string(),
                Some(_) => "Error: expected string for overloaded type".to_string(),
                None => "ExternalValue".to_string(),
            },
            ExternalData(data) => data.read().value_type(),
            Iterator(_) => "Iterator".to_string(),
            TemporaryTuple { .. } => "TemporaryTuple".to_string(),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Empty
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Empty => f.write_str("()"),
            Bool(b) => write!(f, "{}", b),
            Number(n) => write!(f, "{}", n),
            Num2(n) => write!(f, "{}", n),
            Num4(n) => write!(f, "{}", n),
            Str(s) => {
                if f.alternate() {
                    write!(f, "{:#}", s)
                } else {
                    write!(f, "{}", s)
                }
            }
            List(l) => write!(f, "{}", l),
            Tuple(t) => write!(f, "{}", t),
            Map(m) => {
                if f.alternate() {
                    write!(f, "{:#}", m)
                } else {
                    write!(f, "{}", m)
                }
            }
            Range(IntRange { start, end }) => write!(f, "{}..{}", start, end),
            Function(_) => write!(f, "||"),
            Generator(_) => write!(f, "Generator"),
            Iterator(_) => write!(f, "Iterator"),
            ExternalFunction(_) => write!(f, "||"),
            ExternalValue(ref value) => write!(f, "{}", value.data()),
            ExternalData(ref value) => write!(f, "{}", value.read()),
            IndexRange(self::IndexRange { .. }) => f.write_str("IndexRange"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(f, "TemporaryTuple [{}..{}]", start, start + count)
            }
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(value.into())
    }
}

impl From<ExternalValue> for Value {
    fn from(value: ExternalValue) -> Self {
        Self::ExternalValue(value)
    }
}

impl From<ValueIterator> for Value {
    fn from(value: ValueIterator) -> Self {
        Self::Iterator(value)
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeFunction {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
    pub arg_count: u8,
    pub instance_function: bool,
    pub variadic: bool,
    pub captures: Option<ValueList>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexRange {
    pub start: usize,
    pub end: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegisterSlice {
    pub start: u8,
    pub count: u8,
}
