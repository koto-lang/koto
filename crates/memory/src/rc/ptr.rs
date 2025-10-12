use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    rc::Rc,
};

use crate::Address;

/// Provides access to a shared value that is initialized on first use
///
/// This macro will return a value of type `$ty`.
/// On the first use, the value is initialized using `$expr.into()`.
/// Subsequent acccesses return a clone of the stored value.
///
/// # Feature-specific behavior
///
/// - With the "rc" feature, the value is stored in a `thread_local`.
/// - With the "arc" feature, the value is stored in a `static`.
///
/// # Examples
///
/// ```
/// use koto_memory::{ Ptr, lazy };
///
/// fn my_string_constant() -> Ptr<str> {
///     lazy!(Ptr<str>; "foo")
/// }
///
/// let s0 = my_string_constant();
/// let s1 = my_string_constant();
///
/// assert!(Ptr::ptr_eq(&s0, &s1));
/// ```
#[macro_export]
macro_rules! lazy {
    ($ty:ty; $expr:expr) => {{
        thread_local! {
            static VALUE: $ty = $expr.into();
        }
        VALUE.with(Clone::clone)
    }};
}

/// Makes a Ptr, with support for casting to trait objects
///
/// Although `Ptr::from` can be used, the challenge comes when a trait object needs to be used as
/// the pointee type. Until the `CoerceUnized` trait is stabilized, casting from a concrete type to
/// `dyn Trait` needs to be performed on the inner pointer. This macro encapsulates the casting to
/// make life easier at the call site.
#[macro_export]
macro_rules! make_ptr {
    ($value:expr) => {{
        use std::rc::Rc;

        Ptr::from(Rc::new($value) as Rc<_>)
    }};
}

/// An immutable pointer to a value in allocated memory
#[derive(Debug, Default)]
pub struct Ptr<T: ?Sized>(Rc<T>);

impl<T> From<T> for Ptr<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl<T: ?Sized> From<Box<T>> for Ptr<T> {
    fn from(boxed: Box<T>) -> Self {
        Self(boxed.into())
    }
}

impl<T: ?Sized> From<Rc<T>> for Ptr<T> {
    fn from(inner: Rc<T>) -> Self {
        Self(inner)
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

    /// Returns the number of references to the allocated memory
    ///
    /// Only strong references are counted, weak references don't get added to the result.
    pub fn ref_count(this: &Self) -> usize {
        Rc::strong_count(&this.0)
    }
}

impl<T: Clone> Ptr<T> {
    /// Makes a mutable reference into the owned `T`
    ///
    /// If the pointer has the only reference to the value, then the reference will be returned.
    /// Otherwise a clone of the value will be made to ensure uniqueness before returning the
    /// reference.
    ///
    /// See also: [std::rc::Rc::make_mut]
    pub fn make_mut(this: &mut Self) -> &mut T {
        Rc::make_mut(&mut this.0)
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
