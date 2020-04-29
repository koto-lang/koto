use crate::{
    external::{ExternalFunction, ExternalValue},
    value_iterator::{IntRange, ValueIterator},
    value_list::{ValueList, ValueVec},
    value_map::{ValueHashMap, ValueMap},
};
use koto_parser::{vec4, AstFor, AstWhile};
use std::{
    cmp::Ordering,
    fmt,
    iter::FromIterator,
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Vec4(vec4::Vec4),
    Range(IntRange),
    IndexRange { start: usize, end: Option<usize> },
    List(ValueList),
    Map(ValueMap),
    Str(Arc<String>),
    Function(RuntimeFunction),
    VmFunction(VmRuntimeFunction),
    ExternalFunction(ExternalFunction),
    ExternalValue(Arc<RwLock<dyn ExternalValue>>),
    For(Arc<AstFor>),
    While(Arc<AstWhile>),
    Iterator(ValueIterator),
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
            Vec4(v) => write!(f, "({}, {}, {}, {})", v.0, v.1, v.2, v.3),
            Str(s) => f.write_str(&s),
            List(l) => f.write_str(&l.to_string()),
            Map(m) => {
                write!(f, "Map: ")?;
                write!(f, "{{")?;
                let mut first = true;
                for (key, _value) in m.data().iter() {
                    if first {
                        write!(f, " ")?;
                    } else {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", key)?;
                    first = false;
                }
                write!(f, " }}")
            }
            Range(IntRange { start, end }) => write!(f, "[{}..{}]", start, end),
            IndexRange { start, end } => write!(
                f,
                "[{}..{}]",
                start,
                end.map_or("".to_string(), |n| n.to_string()),
            ),
            Function(fun) => {
                let raw = Arc::into_raw(fun.function.clone());
                write!(f, "Function: {:?}", raw)
            }
            VmFunction(function)=> write!(f, "VmFunction: {}", function.ip),
            ExternalFunction(function) => {
                let raw = Arc::into_raw(function.function.clone());
                write!(f, "External function: {:?}", raw)
            }
            ExternalValue(ref value) => f.write_str(&value.read().unwrap().to_string()),
            For(_) => write!(f, "For loop"),
            While(_) => write!(f, "While loop"),
            Iterator(_) => write!(f, "Iterator"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Vec4(a), Vec4(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a.as_ref() == b.as_ref(),
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (
                Range(IntRange {
                    start: start_a,
                    end: end_a,
                }),
                Range(IntRange {
                    start: start_b,
                    end: end_b,
                }),
            ) => start_a == start_b && end_a == end_b,
            (
                IndexRange {
                    start: start_a,
                    end: end_a,
                },
                IndexRange {
                    start: start_b,
                    end: end_b,
                },
            ) => start_a == start_b && end_a == end_b,
            (Function(a), Function(b)) => a == b,
            (VmFunction(a), VmFunction(b)) => a == b,
            (Empty, Empty) => true,
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
            (Vec4(a), Vec4(b)) => a.partial_cmp(b),
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

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmRuntimeFunction {
    pub ip: usize,
    pub arg_count: u8,
    pub is_instance_function: bool,
    pub captures: ValueList,
}

#[derive(Clone, Debug)]
pub struct RuntimeFunction {
    pub function: Arc<koto_parser::Function>,
    pub captured: ValueMap,
}

impl PartialEq for RuntimeFunction {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.function, &other.function) && self.captured == other.captured
    }
}

pub fn copy_value(value: &Value) -> Value {
    use Value::{List, Map};

    match value {
        List(l) => {
            let result = l.data().iter().map(|v| copy_value(v)).collect::<ValueVec>();
            List(ValueList::with_data(result))
        }
        Map(m) => {
            let result =
                ValueHashMap::from_iter(m.data().iter().map(|(k, v)| (k.clone(), copy_value(v))));
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
        Vec4(_) => "Vec4".to_string(),
        List(_) => "List".to_string(),
        Range { .. } => "Range".to_string(),
        IndexRange { .. } => "IndexRange".to_string(),
        Map(_) => "Map".to_string(),
        Str(_) => "String".to_string(),
        Function(_) => "Function".to_string(),
        VmFunction { .. } => "VmFunction".to_string(),
        ExternalFunction(_) => "ExternalFunction".to_string(),
        ExternalValue(value) => value.read().unwrap().value_type(),
        For(_) => "For".to_string(),
        While(_) => "While".to_string(),
        Iterator(_) => "Iterator".to_string(),
    }
}

pub fn make_external_value(value: impl ExternalValue) -> Value {
    Value::ExternalValue(Arc::new(RwLock::new(value)))
}
