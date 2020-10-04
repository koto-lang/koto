use {
    crate::{
        external::{ExternalFunction, ExternalValue},
        value_iterator::{IntRange, ValueIterator},
        value_list::{ValueList, ValueVec},
        value_map::{ValueHashMap, ValueMap},
    },
    koto_bytecode::Chunk,
    koto_parser::{num2, num4},
    std::{
        cmp::Ordering,
        fmt,
        hash::{Hash, Hasher},
        iter::FromIterator,
        sync::{Arc, RwLock},
    },
};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Num2(num2::Num2),
    Num4(num4::Num4),
    Range(IntRange),
    List(ValueList),
    Map(ValueMap),
    Str(Arc<String>),
    Function(RuntimeFunction),
    Iterator(ValueIterator),
    ExternalFunction(ExternalFunction),
    ExternalValue(Arc<RwLock<dyn ExternalValue>>),
    // Internal value types
    IndexRange(IndexRange),
    RegisterList(RegisterList),
    ExternalDataId,
}

#[derive(Clone, Debug)]
pub enum ValueRef<'a> {
    Empty,
    Bool(&'a bool),
    Number(&'a f64),
    Num2(&'a num2::Num2),
    Num4(&'a num4::Num4),
    Range(&'a IntRange),
    List(&'a ValueList),
    Map(&'a ValueMap),
    Str(&'a str),
    Function(&'a RuntimeFunction),
    Iterator(&'a ValueIterator),
    ExternalFunction(&'a ExternalFunction),
    ExternalValue(&'a Arc<RwLock<dyn ExternalValue>>),
    IndexRange(&'a IndexRange),
    RegisterList(&'a RegisterList),
    ExternalDataId,
}

impl Value {
    pub fn as_ref(&self) -> ValueRef {
        match self {
            Value::Empty => ValueRef::Empty,
            Value::Bool(b) => ValueRef::Bool(b),
            Value::Number(n) => ValueRef::Number(n),
            Value::Num2(n) => ValueRef::Num2(n),
            Value::Num4(n) => ValueRef::Num4(n),
            Value::Str(s) => ValueRef::Str(s.as_str()),
            Value::List(l) => ValueRef::List(l),
            Value::Map(m) => ValueRef::Map(m),
            Value::Range(r) => ValueRef::Range(r),
            Value::IndexRange(r) => ValueRef::IndexRange(r),
            Value::Function(f) => ValueRef::Function(f),
            Value::Iterator(i) => ValueRef::Iterator(i),
            Value::ExternalFunction(f) => ValueRef::ExternalFunction(f),
            Value::ExternalValue(v) => ValueRef::ExternalValue(v),
            Value::RegisterList(l) => ValueRef::RegisterList(l),
            Value::ExternalDataId => ValueRef::ExternalDataId,
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
            Str(s) => f.write_str(s),
            List(l) => f.write_str(&l.to_string()),
            Map(m) => f.write_str(&m.to_string()),
            Range(IntRange { start, end }) => write!(f, "[{}..{}]", start, end),
            IndexRange(self::IndexRange { start, end }) => write!(
                f,
                "[{}..{}]",
                start,
                end.map_or("".to_string(), |n| n.to_string()),
            ),
            Function(_) => write!(f, "Function"),
            Iterator(_) => write!(f, "Iterator"),
            ExternalFunction(_) => write!(f, "External Function"),
            ExternalValue(ref value) => f.write_str(&value.read().unwrap().to_string()),
            RegisterList(self::RegisterList { start, count }) => {
                write!(f, "RegisterList [{}..{}]", start, start + count)
            }
            ExternalDataId => write!(f, "External Data ID"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Num2(a), Num2(b)) => a == b,
            (Num4(a), Num4(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a.as_ref() == b.as_ref(),
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (IndexRange(a), IndexRange(b)) => a == b,
            (Function(a), Function(b)) => a == b,
            (Empty, Empty) => true,
            (ExternalDataId, ExternalDataId) => true,
            _ => false,
        }
    }
}

impl<'a> PartialEq for ValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        use ValueRef::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Num2(a), Num2(b)) => a == b,
            (Num4(a), Num4(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (IndexRange(a), IndexRange(b)) => a == b,
            (Function(a), Function(b)) => a == b,
            (Empty, Empty) => true,
            (ExternalDataId, ExternalDataId) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => a.partial_cmp(b),
            (Num2(a), Num2(b)) => a.partial_cmp(b),
            (Num4(a), Num4(b)) => a.partial_cmp(b),
            (Str(a), Str(b)) => a.partial_cmp(b),
            (a, b) => panic!(format!("partial_cmp unsupported for {} and {}", a, b)),
        }
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => match (a.is_nan(), b.is_nan()) {
                (true, true) => Ordering::Equal,
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
                (false, false) => a.partial_cmp(b).unwrap(),
            },
            (Str(a), Str(b)) => a.cmp(b),
            (a, b) => panic!(format!("cmp unsupported for {} and {}", a, b)),
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl<'a> Hash for ValueRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use ValueRef::*;

        std::mem::discriminant(self).hash(state);

        match self {
            Empty | ExternalDataId => {}
            Bool(b) => b.hash(state),
            Number(n) => state.write_u64(n.to_bits()),
            Num2(n) => n.hash(state),
            Num4(n) => n.hash(state),
            Str(s) => s.hash(state),
            Range(IntRange { start, end }) => {
                state.write_isize(*start);
                state.write_isize(*end);
            }
            IndexRange(self::IndexRange { start, end }) => {
                state.write_usize(*start);
                if let Some(end) = end {
                    state.write_usize(*end);
                }
            }
            _ => panic!("Hash is only supported for immutable value types"),
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
        Self::Str(Arc::new(value.to_string()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunction {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
    pub arg_count: u8,
    pub is_instance_function: bool,
    pub is_generator: bool,
    pub captures: Option<ValueList>,
}

impl Hash for RuntimeFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(Arc::as_ptr(&self.chunk) as *const () as usize);
        self.ip.hash(state);
        self.arg_count.hash(state);
        self.is_instance_function.hash(state);
        if let Some(captures) = &self.captures {
            captures.data().hash(state);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexRange {
    pub start: usize,
    pub end: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegisterList {
    pub start: u8,
    pub count: u8,
}

pub fn deep_copy_value(value: &Value) -> Value {
    use Value::{List, Map};

    match value {
        List(l) => {
            let result = l
                .data()
                .iter()
                .map(|v| deep_copy_value(v))
                .collect::<ValueVec>();
            List(ValueList::with_data(result))
        }
        Map(m) => {
            let result = ValueHashMap::from_iter(
                m.data()
                    .iter()
                    .map(|(k, v)| (k.clone(), deep_copy_value(v))),
            );
            Map(ValueMap::with_data(result))
        }
        _ => value.clone(),
    }
}

pub fn type_as_string(value: &Value) -> String {
    use Value::*;
    match &value {
        Empty => "Empty".to_string(),
        Bool(_) => "Bool".to_string(),
        Number(_) => "Number".to_string(),
        Num2(_) => "Num2".to_string(),
        Num4(_) => "Num4".to_string(),
        List(_) => "List".to_string(),
        Range { .. } => "Range".to_string(),
        IndexRange { .. } => "IndexRange".to_string(),
        Map(_) => "Map".to_string(),
        Str(_) => "String".to_string(),
        Function { .. } => "Function".to_string(),
        ExternalFunction(_) => "ExternalFunction".to_string(),
        ExternalValue(value) => value.read().unwrap().value_type(),
        Iterator(_) => "Iterator".to_string(),
        RegisterList { .. } => "RegisterList".to_string(),
        ExternalDataId => "ExternalDataId".to_string(),
    }
}

pub fn make_external_value(value: impl ExternalValue) -> Value {
    Value::ExternalValue(Arc::new(RwLock::new(value)))
}

pub fn value_is_immutable(value: &Value) -> bool {
    use Value::*;

    matches!(
        value,
        Empty | ExternalDataId | Bool(_) | Number(_) | Num2(_) | Num4(_) | Range(_) | Str(_))
}
