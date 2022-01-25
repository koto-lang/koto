use {
    crate::{
        num2, num4, value_key::ValueRef, value_map::ValueMap, ExternalData, ExternalFunction,
        ExternalValue, MetaKey, ValueIterator, ValueList, ValueNumber, ValueString, ValueTuple,
        ValueVec,
    },
    koto_bytecode::Chunk,
    std::{cell::RefCell, fmt, rc::Rc},
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

    /// A callable function with simple properties
    SimpleFunction(SimpleFunctionInfo),

    /// A callable function with less simple properties, e.g. captures, instance function, etc.
    Function(FunctionInfo),

    /// A function that produces an Iterator when called
    ///
    /// A [Vm](crate::Vm) gets spawned for the function to run in, which pauses each time a yield
    /// instruction is encountered. See Vm::call_generator and Iterable::Generator.
    Generator(FunctionInfo),

    /// The iterator type used in Koto
    Iterator(ValueIterator),

    /// A function that's defined outside of the Koto runtime
    ExternalFunction(ExternalFunction),

    /// A value type that's defined outside of the Koto runtime
    ExternalValue(ExternalValue),

    /// A 'data-only' counterpart to ExternalValue
    ExternalData(Rc<RefCell<dyn ExternalData>>),

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

    /// The builder used while building lists or tuples
    ///
    /// Note: this is intended for internal use only.
    SequenceBuilder(Vec<Value>),

    /// The string builder used during string interpolation
    ///
    /// Note: this is intended for internal use only.
    StringBuilder(String),
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
            Value::Str(s) => ValueRef::Str(s),
            Value::Range(r) => ValueRef::Range(r),
            _ => unreachable!(), // Only immutable values can be used in ValueKey
        }
    }

    #[must_use]
    pub fn deep_copy(&self) -> Value {
        use Value::*;

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
            Iterator(i) => Iterator(i.make_copy()),
            _ => self.clone(),
        }
    }

    pub fn is_callable(&self) -> bool {
        use Value::*;
        matches!(self, SimpleFunction(_) | Function(_) | ExternalFunction(_))
    }

    pub fn is_immutable(&self) -> bool {
        use Value::*;
        matches!(
            self,
            Empty | Bool(_) | Number(_) | Num2(_) | Num4(_) | Range(_) | Str(_)
        )
    }

    /// Returns true if a `ValueIterator` can be made from the value
    pub fn is_iterable(&self) -> bool {
        use Value::*;
        matches!(
            self,
            Num2(_) | Num4(_) | Range(_) | List(_) | Tuple(_) | Map(_) | Str(_) | Iterator(_)
        )
    }

    /// Returns the 'size' of the value
    ///
    /// A value's size is the number of elements that can used in unpacking expressions
    /// e.g.
    /// x = [1, 2, 3] # x has size 3
    /// a, b, c = x
    ///
    /// See [Op::Size](koto_bytecode::Op::Size) and [Op::CheckSize](koto_bytecode::Op::CheckSize).
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
            SimpleFunction(_) => "Function".to_string(),
            Function(_) => "Function".to_string(),
            Generator(_) => "Generator".to_string(),
            ExternalFunction(_) => "ExternalFunction".to_string(),
            ExternalValue(value) => match value.meta().get(&MetaKey::Type) {
                Some(Str(s)) => s.as_str().to_string(),
                Some(_) => "Error: expected string for overloaded type".to_string(),
                None => "ExternalValue".to_string(),
            },
            ExternalData(data) => data.borrow().value_type(),
            Iterator(_) => "Iterator".to_string(),
            TemporaryTuple { .. } => "TemporaryTuple".to_string(),
            SequenceBuilder(_) => "SequenceBuilder".to_string(),
            StringBuilder(_) => "StringBuilder".to_string(),
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
            Bool(b) => write!(f, "{b}"),
            Number(n) => write!(f, "{n}"),
            Num2(n) => write!(f, "{n}"),
            Num4(n) => write!(f, "{n}"),
            Str(s) => {
                if f.alternate() {
                    write!(f, "{s:#}")
                } else {
                    write!(f, "{s}")
                }
            }
            List(l) => write!(f, "{l}"),
            Tuple(t) => write!(f, "{t}"),
            Map(m) => {
                if f.alternate() {
                    write!(f, "{m:#}")
                } else {
                    write!(f, "{m}")
                }
            }
            Range(IntRange { start, end }) => write!(f, "{start}..{end}"),
            SimpleFunction(_) | Function(_) => write!(f, "||"),
            Generator(_) => write!(f, "Generator"),
            Iterator(_) => write!(f, "Iterator"),
            ExternalFunction(_) => write!(f, "||"),
            ExternalValue(_) | ExternalData(_) => f.write_str(&self.type_as_string()),
            IndexRange(self::IndexRange { .. }) => f.write_str("IndexRange"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(f, "TemporaryTuple [{start}..{}]", start + count)
            }
            SequenceBuilder(_) => write!(f, "SequenceBuilder"),
            StringBuilder(s) => write!(f, "StringBuilder({s})"),
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
pub struct SimpleFunctionInfo {
    /// The [Chunk] in which the function can be found.
    pub chunk: Rc<Chunk>,
    /// The start ip of the function.
    pub ip: usize,
    /// The expected number of arguments for the function
    pub arg_count: u8,
}

#[derive(Clone, Debug)]
pub struct FunctionInfo {
    /// The [Chunk] in which the function can be found.
    pub chunk: Rc<Chunk>,
    /// The start ip of the function.
    pub ip: usize,
    /// The expected number of arguments for the function.
    pub arg_count: u8,
    /// If the function is an instance function, then the first argument will be `self`.
    pub instance_function: bool,
    /// If the function is variadic, then extra args will be captured in a tuple.
    pub variadic: bool,
    /// If the function has a single arg, and that arg is an unpacked tuple
    ///
    /// This is used to optimize external calls where the caller has a series of args that might be
    /// unpacked by the function, and it would be wasteful to create a Tuple when it's going to be
    /// immediately unpacked and discarded.
    pub arg_is_unpacked_tuple: bool,
    /// The optional list of captures that should be copied into scope when the function is called.
    //
    // Q. Why use a ValueList?
    // A. Because capturing values currently works by assigning by index, after the function
    //    itself has been created.
    // Q. Why not use a SequenceBuilder?
    // A. Recursive functions need to capture themselves into the list, and the captured function
    //    and the assigned function need to share the same captures list. Currently the only way
    //    for this to work is to allow mutation of the shared list after the creation of the
    //    function, so a ValueList is a reasonable choice.
    // Q. What about using Rc<[Value]> for non-recursive functions, or Option<Value> for
    //    non-recursive functions with a single capture?
    // A. These could be potential optimizations to investigate at some point, but would involve
    //    placing FunctionInfo behind an Rc due to its increased size, so it's not clear if there
    //    would be an overall performance win.
    pub captures: Option<ValueList>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

impl IntRange {
    pub fn is_ascending(&self) -> bool {
        self.start <= self.end
    }

    pub fn len(&self) -> usize {
        if self.is_ascending() {
            (self.end - self.start) as usize
        } else {
            (self.start - self.end) as usize
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_mem_size() {
        // All Value variants should have a size of <= 32 bytes, and with the variant flag the
        // total size of Value should not be greater than 40 bytes.
        assert!(std::mem::size_of::<Value>() <= 40);
    }
}
