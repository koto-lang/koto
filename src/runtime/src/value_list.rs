use crate::Value;
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub type ValueVec = smallvec::SmallVec<[Value; 4]>;

#[derive(Clone, Debug, Default)]
pub struct ValueList(Arc<RwLock<ValueVec>>);

impl ValueList {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(RwLock::new(ValueVec::with_capacity(capacity))))
    }

    #[inline]
    pub fn with_data(data: ValueVec) -> Self {
        Self(Arc::new(RwLock::new(data)))
    }

    #[inline]
    pub fn from_slice(data: &[Value]) -> Self {
        Self(Arc::new(RwLock::new(
            data.iter().cloned().collect::<ValueVec>(),
        )))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn data(&self) -> RwLockReadGuard<ValueVec> {
        self.0.read().unwrap()
    }

    #[inline]
    pub fn data_mut(&self) -> RwLockWriteGuard<ValueVec> {
        self.0.write().unwrap()
    }
}

impl fmt::Display for ValueList {
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

impl PartialEq for ValueList {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        *self.data() == *other.data()
    }
}
