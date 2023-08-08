use std::{fmt, ops::Deref, rc::Rc};

use super::Address;

/// An immutable pointer to a value in allocated memory
#[derive(Debug, Default)]
pub struct Ptr<T: ?Sized>(Rc<T>);

impl<T> Ptr<T> {
    /// Moves the value into newly allocated memory
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }
}

impl<T: ?Sized> Ptr<T> {
    /// Returns true if the two `Ptr`s point to the same allocation
    ///
    /// See also: [std::rc::Rc::ptr_eq]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(&this.0, &other.0)
    }

    /// Returns the address of the allocated memory
    pub fn address(this: &Self) -> Address {
        Rc::as_ptr(&this.0).into()
    }
}

impl<T> From<T> for Ptr<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> From<Rc<T>> for Ptr<T> {
    fn from(inner: Rc<T>) -> Self {
        Self(inner)
    }
}

impl<T: ?Sized> Deref for Ptr<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<T: ?Sized> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Clone> From<&[T]> for Ptr<[T]> {
    #[inline]
    fn from(value: &[T]) -> Self {
        Self(Rc::from(value))
    }
}

impl<T> From<Vec<T>> for Ptr<[T]> {
    #[inline]
    fn from(value: Vec<T>) -> Self {
        Self(Rc::from(value))
    }
}

impl From<&str> for Ptr<str> {
    #[inline]
    fn from(value: &str) -> Self {
        Self(Rc::from(value))
    }
}

impl From<String> for Ptr<str> {
    #[inline]
    fn from(value: String) -> Self {
        Self(Rc::from(value))
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Ptr<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::eq(&self.0, &other.0)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Ptr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
