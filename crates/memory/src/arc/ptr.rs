use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use crate::Address;

/// Makes a Ptr, with support for casting to trait objects
///
/// Although Ptr::new can be used, the challenge comes when a trait object needs to be used as
/// the pointee type. Until the `CoerceUnized` trait is stabilized, casting from a concrete type to
/// `dyn Trait` needs to be performed on the inner pointer. This macro encapsulates the casting to
/// make life easier at the call site.
#[macro_export]
macro_rules! make_ptr {
    ($value:expr) => {{
        use std::sync::Arc;

        Ptr::from(Arc::new($value) as Arc<_>)
    }};
}

/// An immutable pointer to a value in allocated memory
#[derive(Debug, Default)]
pub struct Ptr<T: ?Sized>(Arc<T>);

impl<T> Ptr<T> {
    /// Moves the provided value into newly allocated memory
    pub fn new(value: T) -> Self {
        Self::from(value)
    }
}

impl<T: ?Sized> Ptr<T> {
    /// Returns true if the two `Ptr`s point to the same allocation
    ///
    /// See also: [std::sync::Arc::ptr_eq]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Arc::ptr_eq(&this.0, &other.0)
    }

    /// Returns the address of the allocated memory
    pub fn address(this: &Self) -> Address {
        Arc::as_ptr(&this.0).into()
    }

    /// Returns the number of references to the allocated memory
    ///
    /// Only strong references are counted, weak references don't get added to the result.
    pub fn ref_count(this: &Self) -> usize {
        Arc::strong_count(&this.0)
    }
}

impl<T: Clone> Ptr<T> {
    /// Makes a mutable reference into the owned `T`
    ///
    /// If the pointer has the only reference to the value, then the reference will be returned.
    /// Otherwise a clone of the value will be made to ensure uniqueness before returning the
    /// reference.
    ///
    /// See also: [std::sync::Arc::make_mut]
    pub fn make_mut(this: &mut Self) -> &mut T {
        Arc::make_mut(&mut this.0)
    }
}

impl<T> From<T> for Ptr<T> {
    fn from(value: T) -> Self {
        Self(Arc::from(value))
    }
}

impl<T: ?Sized> From<Box<T>> for Ptr<T> {
    fn from(boxed: Box<T>) -> Self {
        Self(boxed.into())
    }
}

impl<T: ?Sized> From<Arc<T>> for Ptr<T> {
    fn from(inner: Arc<T>) -> Self {
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
        Self(Arc::clone(&self.0))
    }
}

impl<T: Clone> From<&[T]> for Ptr<[T]> {
    #[inline]
    fn from(value: &[T]) -> Self {
        Self(Arc::from(value))
    }
}

impl<T> From<Vec<T>> for Ptr<[T]> {
    #[inline]
    fn from(value: Vec<T>) -> Self {
        Self(Arc::from(value))
    }
}

impl From<&str> for Ptr<str> {
    #[inline]
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for Ptr<str> {
    #[inline]
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Ptr<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::eq(&self.0, &other.0)
    }
}

impl<T: ?Sized + Eq> Eq for Ptr<T> {}

impl<T: ?Sized + fmt::Display> fmt::Display for Ptr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ?Sized + Hash> Hash for Ptr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T: ?Sized + Ord> Ord for Ptr<T> {
    #[inline]
    fn cmp(&self, other: &Ptr<T>) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: ?Sized + PartialOrd> PartialOrd for Ptr<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
