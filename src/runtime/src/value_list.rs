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

    pub fn len(&self) -> usize {
        self.data().len()
    }

    pub fn data(&self) -> Ref<ValueVec<'a>> {
        self.0.borrow()
    }

    pub fn data_mut(&self) -> RefMut<ValueVec<'a>> {
        self.0.borrow_mut()
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
