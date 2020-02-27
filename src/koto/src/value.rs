use crate::value_map::ValueMap;
use koto_parser::{vec4, AstFor, Function};
use std::{cell::RefCell, cmp::Ordering, fmt, rc::Rc};

#[derive(Clone, Debug)]
pub enum Value<'a> {
    Empty,
    Bool(bool),
    Number(f64),
    Vec4(vec4::Vec4),
    List(Rc<Vec<Value<'a>>>),
    Range { min: isize, max: isize },
    Map(Rc<ValueMap<'a>>),
    Str(Rc<String>),
    Function(Rc<Function>),
    ExternalFunction(ExternalFunction<'a>),
    For(Rc<AstFor>),
}

impl<'a> fmt::Display for Value<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Empty => write!(f, "()"),
            Bool(s) => write!(f, "{}", s),
            Number(n) => write!(f, "{}", n),
            Vec4(v) => write!(f, "({}, {}, {}, {})", v.0, v.1, v.2, v.3),
            Str(s) => write!(f, "{}", s),
            List(a) => {
                write!(f, "[")?;
                for (i, value) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Map(m) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in m.0.iter() {
                    if first {
                        write!(f, " ")?;
                    } else {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, value)?;
                    first = false;
                }
                write!(f, " }}")
            }
            Range { min, max } => write!(f, "[{}..{}]", min, max),
            Function(function) => {
                let raw = Rc::into_raw(function.clone());
                write!(f, "function: {:?}", raw)
            }
            ExternalFunction(function) => {
                let raw = Rc::into_raw(function.0.clone());
                write!(f, "builtin function: {:?}", raw)
            }
            For(_) => write!(f, "For loop"),
        }
    }
}

impl<'a> PartialEq for Value<'a> {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Vec4(a), Vec4(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a.as_ref() == b.as_ref(),
            (List(a), List(b)) => a.as_ref() == b.as_ref(),
            (Map(a), Map(b)) => a.as_ref() == b.as_ref(),
            (Function(a), Function(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}
impl<'a> Eq for Value<'a> {}

impl<'a> PartialOrd for Value<'a> {
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

impl<'a> Ord for Value<'a> {
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

impl<'a> From<bool> for Value<'a> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

pub type BuiltinResult<'a> = Result<Value<'a>, String>;
pub struct ExternalFunction<'a>(pub Rc<RefCell<dyn FnMut(&[Value<'a>]) -> BuiltinResult<'a> + 'a>>);

impl<'a> Clone for ExternalFunction<'a> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a> fmt::Debug for ExternalFunction<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = Rc::into_raw(self.0.clone());
        write!(f, "builtin function: {:?}", raw)
    }
}

pub(super) struct ValueIterator<'a> {
    value: Value<'a>,
    index: isize,
}

impl<'a> ValueIterator<'a> {
    pub fn new(value: Value<'a>) -> Self {
        Self { value, index: 0 }
    }
}

impl<'a> Iterator for ValueIterator<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Value<'a>> {
        use Value::*;

        let result = match &self.value {
            List(a) => a.get(self.index as usize).cloned(),
            Range { min, max } => {
                if self.index < (max - min) {
                    Some(Number((min + self.index) as f64))
                } else {
                    None
                }
            }
            _ => None,
        };

        if result.is_some() {
            self.index += 1;
        }

        result
    }
}

pub(super) struct MultiRangeValueIterator<'a>(pub Vec<ValueIterator<'a>>);

impl<'a> Iterator for MultiRangeValueIterator<'a> {
    type Item = Vec<Value<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.iter_mut().map(Iterator::next).collect()
    }
}

pub fn type_as_string(value: &Value) -> &'static str {
    use Value::*;
    match value {
        Empty => "Empty",
        Bool(_) => "Bool",
        Number(_) => "Number",
        Vec4(_) => "Vec4",
        List(_) => "List",
        Range { .. } => "Range",
        Map(_) => "Map",
        Str(_) => "String",
        Function(_) => "Function",
        ExternalFunction(_) => "ExternalFunction",
        For(_) => "For",
    }
}
