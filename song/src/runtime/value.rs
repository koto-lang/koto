use crate::parser::{AstFor, Function, Id};
use std::{collections::HashMap, fmt, rc::Rc};


#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Array(Rc<Vec<Value>>),
    Range { min: isize, max: isize },
    Map(Rc<HashMap<Id, Value>>),
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
    For(Rc<AstFor>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Empty => write!(f, "()"),
            Bool(s) => write!(f, "{}", s),
            Number(n) => write!(f, "{}", n),
            StrLiteral(s) => write!(f, "{}", s),
            Array(a) => {
                write!(f, "[")?;
                for (i, value) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Map(t) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in t.iter() {
                    if first {
                        write!(f, " ")?;
                    }
                    else {
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
            // BuiltinFunction(function) => {
            //     let raw = Rc::into_raw(function.clone());
            //     write!(f, "builtin function: {:?}", raw)
            // }
            _ => unreachable!(),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Array(a), Array(b)) => a.as_ref() == b.as_ref(),
            (Map(a), Map(b)) => a.as_ref() == b.as_ref(),
            (Function(a), Function(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

pub(super) struct ValueIterator {
    value: Value,
    index: isize,
}

impl ValueIterator {
    pub fn new(value: Value) -> Self {
        Self { value, index: 0 }
    }
}

impl Iterator for ValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        use Value::*;

        let result = match &self.value {
            Array(a) => a.get(self.index as usize).cloned(),
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

pub(super) struct MultiRangeValueIterator(pub Vec<ValueIterator>);

impl Iterator for MultiRangeValueIterator {
    type Item = Vec<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.iter_mut().map(Iterator::next).collect()
    }
}
