use {
    crate::Value,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        rc::Rc,
    },
};

/// The underlying Vec type used by [ValueList]
pub type ValueVec = smallvec::SmallVec<[Value; 4]>;

/// The Koto runtime's List type
#[derive(Clone, Debug, Default)]
pub struct ValueList(Rc<RefCell<ValueVec>>);

impl ValueList {
    /// Creates an empty list with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Rc::new(RefCell::new(ValueVec::with_capacity(capacity))))
    }

    /// Creates a list containing the provided data
    pub fn with_data(data: ValueVec) -> Self {
        Self(Rc::new(RefCell::new(data)))
    }

    /// Creates a list containing the provided slice of [Values](crate::Value)
    pub fn from_slice(data: &[Value]) -> Self {
        Self(Rc::new(RefCell::new(
            data.iter().cloned().collect::<ValueVec>(),
        )))
    }

    /// Returns the number of entries of the list
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Returns true if there are no entries in the list
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a reference to the list's entries
    pub fn data(&self) -> Ref<ValueVec> {
        self.0.borrow()
    }

    /// Returns a mutable reference to the list's entries
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
