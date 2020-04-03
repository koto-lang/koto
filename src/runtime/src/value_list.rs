use crate::Value;

use std::fmt;

// pub type ValueVec<'a> = Vec<Value<'a>>;
pub type ValueVec<'a> = smallvec::SmallVec<[Value<'a>; 4]>;

#[derive(Clone, Debug, Default)]
pub struct ValueList<'a>(ValueVec<'a>);

impl<'a> ValueList<'a> {
    pub fn new() -> Self {
        Self(ValueVec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueVec::with_capacity(capacity))
    }

    pub fn with_data(data: ValueVec<'a>) -> Self {
        Self(data)
    }

    pub fn from_slice(data: &[Value<'a>]) -> Self {
        Self(data.iter().cloned().collect::<ValueVec>())
    }

    pub fn data(&self) -> &ValueVec<'a> {
        &self.0
    }

    pub fn data_mut(&mut self) -> &mut ValueVec<'a> {
        &mut self.0
    }

    pub fn make_mut(&mut self, index: usize) -> Value<'a> {
        let value = &mut self.0[index];
        match value {
            Value::Map(entry) => {
                entry.make_unique();
            }
            Value::List(entry) => {
                entry.make_unique();
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
