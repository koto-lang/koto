use crate::{RcCell, Value};
use std::{
    cell::{Ref, RefMut},
    fmt,
};

pub type ValueVec<'a> = smallvec::SmallVec<[Value<'a>; 4]>;

#[derive(Clone, Debug)]
pub struct ValueList<'a>(RcCell<ValueVec<'a>>);

impl<'a> ValueList<'a> {
    pub fn new() -> Self {
        Self(RcCell::new(ValueVec::new()))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(RcCell::new(ValueVec::with_capacity(capacity)))
    }

    pub fn with_data(data: ValueVec<'a>) -> Self {
        Self(RcCell::new(data))
    }

    pub fn from_slice(data: &[Value<'a>]) -> Self {
        Self(RcCell::new(data.iter().cloned().collect::<ValueVec>()))
    }

    pub fn len(&self) -> usize{
        self.data().len()
    }

    pub fn data(&self) -> Ref<ValueVec<'a>> {
        self.0.borrow()
    }

    pub fn data_mut(&self) -> RefMut<ValueVec<'a>> {
        self.0.borrow_mut()
    }

    pub fn make_unique(&mut self) {
        self.0.make_unique()
    }

    pub fn make_element_unique(&self, index: usize) -> Value<'a> {
        let value = &mut self.data_mut()[index];
        match value {
            Value::Map(element) => {
                element.make_unique();
            }
            Value::List(element) => {
                element.make_unique();
            }
            _ => {}
        }
        value.clone()
    }
}

impl<'a> fmt::Display for ValueList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, value) in self.data().iter().enumerate() {
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
