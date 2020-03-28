use crate::Value;

use std::{fmt, rc::Rc};

#[derive(Clone, Debug, Default)]
pub struct ValueList<'a>(Vec<Value<'a>>);

impl<'a> ValueList<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn with_data(data: Vec<Value<'a>>) -> Self {
        Self(data)
    }

    pub fn data(&self) -> &Vec<Value<'a>> {
        &self.0
    }

    pub fn data_mut(&mut self) -> &mut Vec<Value<'a>> {
        &mut self.0
    }

    pub fn make_mut(&mut self, index: usize) -> Value<'a> {
        let value = &mut self.0[index];
        match value {
            Value::Map(entry) => {
                Rc::make_mut(entry);
            }
            Value::List(entry) => {
                Rc::make_mut(entry);
            }
            _ => {}
        }
        value.clone()
    }
}

impl<'a> fmt::Display for ValueList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, value) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", value)?;
        }
        write!(f, "]")
    }
}

impl<'a> PartialEq for ValueList<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
