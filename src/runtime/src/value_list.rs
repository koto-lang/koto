use crate::Value;
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub type ValueVec<'a> = smallvec::SmallVec<[Value<'a>; 4]>;

#[derive(Clone, Debug)]
pub struct ValueList<'a>(Arc<RwLock<ValueVec<'a>>>);

impl<'a> ValueList<'a> {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ValueVec::new())))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(RwLock::new(ValueVec::with_capacity(capacity))))
    }

    pub fn with_data(data: ValueVec<'a>) -> Self {
        Self(Arc::new(RwLock::new(data)))
    }

    pub fn from_slice(data: &[Value<'a>]) -> Self {
        Self(Arc::new(RwLock::new(
            data.iter().cloned().collect::<ValueVec>(),
        )))
    }

    pub fn len(&self) -> usize {
        self.data().len()
    }

    pub fn data(&self) -> RwLockReadGuard<ValueVec<'a>> {
        self.0.read().unwrap()
    }

    pub fn data_mut(&self) -> RwLockWriteGuard<ValueVec<'a>> {
        self.0.write().unwrap()
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
        *self.data() == *other.data()
    }
}
