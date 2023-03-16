use std::{cell::RefCell, ops::Deref, rc::Rc};

/// A wrapper that simplifies using `Rc<RefCell<T>>`
///
/// Deref is implemented for the inner RefCell, providing access to borrow()/borrow_mut().
///
/// From is used as the standard means of producing an RcCell. To store trait objects in an RcCell,
/// an implementation of From is needed for the trait that's being stored.
#[derive(Debug, Default)]
pub struct RcCell<T: ?Sized>(Rc<RefCell<T>>);

impl<T> From<T> for RcCell<T> {
    fn from(value: T) -> Self {
        Self(Rc::new(RefCell::new(value)))
    }
}

impl<T: ?Sized> From<Rc<RefCell<T>>> for RcCell<T> {
    fn from(inner: Rc<RefCell<T>>) -> Self {
        Self(inner)
    }
}

impl<T: ?Sized> Deref for RcCell<T> {
    type Target = RefCell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> Clone for RcCell<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
