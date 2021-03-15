use {
    crate::{
        num2, num4,
        value_map::{ValueMap, ValueMapContents},
        ExternalFunction, ExternalValue, IntRange, MetaKey, ValueIterator, ValueList, ValueNumber,
        ValueRef, ValueString, ValueTuple, ValueVec,
    },
    koto_bytecode::Chunk,
    parking_lot::RwLock,
    std::{fmt, sync::Arc},
};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(ValueNumber),
    Num2(num2::Num2),
    Num4(num4::Num4),
    Range(IntRange),
    List(ValueList),
    Tuple(ValueTuple),
    Map(ValueMap),
    Str(ValueString),
    Function(RuntimeFunction),
    Generator(RuntimeFunction),
    Iterator(ValueIterator),
    ExternalFunction(ExternalFunction),
    ExternalValue(Arc<RwLock<dyn ExternalValue>>),
    // Internal value types
    IndexRange(IndexRange),
    TemporaryTuple(RegisterSlice),
    ExternalDataId,
}

impl Value {
    #[inline]
    pub fn as_ref(&self) -> ValueRef {
        match &self {
            Value::Empty => ValueRef::Empty,
            Value::Bool(b) => ValueRef::Bool(b),
            Value::Number(n) => ValueRef::Number(n),
            Value::Num2(n) => ValueRef::Num2(n),
            Value::Num4(n) => ValueRef::Num4(n),
            Value::Str(s) => ValueRef::Str(&s),
            Value::Range(r) => ValueRef::Range(r),
            Value::ExternalDataId => ValueRef::ExternalDataId,
            _ => unreachable!(), // Only immutable values can be used in ValueKey
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
            Bool(b) => f.write_str(&b.to_string()),
            Number(n) => f.write_str(&n.to_string()),
            Num2(n) => f.write_str(&n.to_string()),
            Num4(n) => f.write_str(&n.to_string()),
            Str(s) => {
                if f.alternate() {
                    write!(f, "\"{}\"", s)
                } else {
                    f.write_str(s)
                }
            }
            List(l) => f.write_str(&l.to_string()),
            Tuple(t) => f.write_str(&t.to_string()),
            Map(m) => f.write_str(&m.to_string()),
            Range(IntRange { start, end }) => write!(f, "{}..{}", start, end),
            Function(_) => write!(f, "||"),
            Generator(_) => write!(f, "Generator"),
            Iterator(_) => write!(f, "Iterator"),
            ExternalFunction(_) => write!(f, "||"),
            ExternalValue(ref value) => f.write_str(&value.read().to_string()),
            IndexRange(self::IndexRange { .. }) => f.write_str("IndexRange"),
            TemporaryTuple(RegisterSlice { start, count }) => {
                write!(f, "TemporaryTuple [{}..{}]", start, start + count)
            }
            ExternalDataId => write!(f, "External Data"),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
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

pub fn deep_copy_value(value: &Value) -> Value {
    use Value::{List, Map, Tuple};

    match value {
        List(l) => {
            let result = l
                .data()
                .iter()
                .map(|v| deep_copy_value(v))
                .collect::<ValueVec>();
            List(ValueList::with_data(result))
        }
        Tuple(t) => {
            let result = t
                .data()
                .iter()
                .map(|v| deep_copy_value(v))
                .collect::<Vec<_>>();
            Tuple(result.into())
        }
        Map(m) => {
            let data = m
                .contents()
                .data
                .iter()
                .map(|(k, v)| (k.clone(), deep_copy_value(v)))
                .collect();
            let meta = m.contents().meta.clone();
            Map(ValueMap::with_contents(ValueMapContents { data, meta }))
        }
        _ => value.clone(),
    }
}

pub fn type_as_string(value: &Value) -> String {
    use Value::*;
    match &value {
        Empty => "Empty".to_string(),
        Bool(_) => "Bool".to_string(),
        Number(ValueNumber::F64(_)) => "Float".to_string(),
        Number(ValueNumber::I64(_)) => "Int".to_string(),
        Num2(_) => "Num2".to_string(),
        Num4(_) => "Num4".to_string(),
        List(_) => "List".to_string(),
        Range { .. } => "Range".to_string(),
        IndexRange { .. } => "IndexRange".to_string(),
        Map(m) => match m.contents().meta.get(&MetaKey::Type) {
            Some(Str(s)) => s.as_str().to_string(),
            Some(_) => "Error: expected string for overloaded type".to_string(),
            None => "Map".to_string(),
        },
        Str(_) => "String".to_string(),
        Tuple(_) => "Tuple".to_string(),
        Function { .. } => "Function".to_string(),
        Generator { .. } => "Generator".to_string(),
        ExternalFunction(_) => "ExternalFunction".to_string(),
        ExternalValue(value) => value.read().value_type(),
        Iterator(_) => "Iterator".to_string(),
        TemporaryTuple { .. } => "TemporaryTuple".to_string(),
        ExternalDataId => "ExternalDataId".to_string(),
    }
}

pub fn make_external_value(value: impl ExternalValue) -> Value {
    Value::ExternalValue(Arc::new(RwLock::new(value)))
}

pub fn value_is_callable(value: &Value) -> bool {
    use Value::*;
    matches!(value, Function { .. } | ExternalFunction(_))
}

pub fn value_is_iterable(value: &Value) -> bool {
    use Value::*;
    matches!(
        value,
        Range(_) | List(_) | Tuple(_) | Map(_) | Str(_) | Iterator(_)
    )
}

pub fn value_size(value: &Value) -> usize {
    use Value::*;

    match value {
        List(l) => l.len(),
        Str(s) => s.len(),
        Tuple(t) => t.data().len(),
        TemporaryTuple(RegisterSlice { count, .. }) => *count as usize,
        Map(m) => m.len(),
        Num2(_) => 2,
        Num4(_) => 4,
        Range(IntRange { start, end }) => (end - start) as usize,
        _ => 1,
    }
}
