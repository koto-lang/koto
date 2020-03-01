use crate::value_map::ValueMap;
use koto_parser::{vec4, AstFor, Function};
use std::{cell::RefCell, cmp::Ordering, fmt, ops::Deref, rc::Rc};

#[derive(Clone, Debug)]
pub enum Value<'a> {
    Empty,
    Bool(bool),
    Number(f64),
    Vec4(vec4::Vec4),
    List(Rc<Vec<Value<'a>>>),
    Range { min: isize, max: isize },
    Map(Rc<RefCell<ValueMap<'a>>>),
    Str(Rc<String>),
    Ref(Rc<RefCell<Value<'a>>>),
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
                for (key, value) in m.borrow().0.iter() {
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
            Ref(r) => {
                let value = r.borrow();
                write!(f, "Reference to {}{}", type_as_string(&value), value)
            }
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
            (Ref(a), Ref(b)) => a.as_ref() == b.as_ref(),
            (Ref(a), _) => a.borrow().deref() == other,
            (_, Ref(b)) => self == b.borrow().deref(),
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

pub fn values_have_matching_type<'a>(a: &Value<'a>, b: &Value<'a>) -> bool {
    use std::mem::discriminant;
    use Value::Ref;

    match (a, b) {
        (Ref(a), Ref(b)) => discriminant(a.borrow().deref()) == discriminant(b.borrow().deref()),
        (Ref(a), _) => discriminant(a.borrow().deref()) == discriminant(b),
        (_, Ref(b)) => discriminant(a) == discriminant(b.borrow().deref()),
        (_, _) => discriminant(a) == discriminant(b),
    }
}

pub fn deref_value<'a>(value: &Value<'a>) -> Value<'a> {
    use Value::Ref;

    match value {
        Ref(r) => r.borrow().clone(),
        _ => value.clone(),
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
        Ref(_) => "Reference",
        Function(_) => "Function",
        ExternalFunction(_) => "ExternalFunction",
        For(_) => "For",
    }
}
