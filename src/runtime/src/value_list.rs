use {
    crate::Value,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        rc::Rc,
    },
};

pub type ValueVec = smallvec::SmallVec<[Value; 4]>;

#[derive(Clone, Debug, Default)]
pub struct ValueList(Rc<RefCell<ValueVec>>);

impl ValueList {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Rc::new(RefCell::new(ValueVec::with_capacity(capacity))))
    }

    #[inline]
    pub fn with_data(data: ValueVec) -> Self {
        Self(Rc::new(RefCell::new(data)))
    }

    #[inline]
    pub fn from_slice(data: &[Value]) -> Self {
        Self(Rc::new(RefCell::new(
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
    pub fn data(&self) -> Ref<ValueVec> {
        self.0.borrow()
    }

    #[inline]
    pub fn data_mut(&self) -> RefMut<ValueVec> {
        self.0.borrow_mut()
    }
}

impl fmt::Display for ValueList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, value) in self.data().iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{value:#}")?;
        }
        write!(f, "]")
    }
}
