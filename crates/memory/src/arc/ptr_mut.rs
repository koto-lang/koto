use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use crate::Ptr;

/// Makes a PtrMut, with support for casting to trait objects
///
/// Although PtrMut::from is available, the challenge comes when a trait object needs to be used as
/// the pointee type. Until the `CoerceUnized` trait is stabilized, casting from a concrete type to
/// `dyn Trait` needs to be performed on the inner pointer. This macro encapsulates the casting to
/// make life easier at the call site.
#[macro_export]
macro_rules! make_ptr_mut {
    ($value:expr) => {{
        use std::sync::Arc;

        PtrMut::from(Arc::from(KCell::from($value)) as Arc<KCell<_>>)
    }};
}

/// A mutable pointer to a value in allocated memory
pub type PtrMut<T> = Ptr<KCell<T>>;

impl<T> From<T> for PtrMut<T> {
    fn from(value: T) -> Self {
        Ptr::from(KCell::from(value))
    }
}

/// A mutable value with borrowing checked at runtime
#[derive(Debug, Default)]
pub struct KCell<T: ?Sized>(RwLock<T>);

impl<T> From<T> for KCell<T> {
    fn from(value: T) -> Self {
        Self(RwLock::from(value))
    }
}

impl<T: ?Sized> KCell<T> {
    /// Immutably borrows the wrapped value.
    ///
    /// Multiple immutable borrows can be made at the same time.
    ///
    /// If the value is currently mutably borrowed then this function will block.
    /// See `try_borrow` for a non-blocking version.
    pub fn borrow(&self) -> Borrow<T> {
        Borrow::new(self.0.read())
    }

    /// Attempts to mutably borrow the wrapped value.
    ///
    /// Returns an error if the value is currently mutably borrowed.
    pub fn try_borrow(&self) -> Option<Borrow<'_, T>> {
        self.0.try_read().map(Borrow::new)
    }

    /// Mutably borrows the wrapped value.
    ///
    /// If the value is currently borrowed then this function will block until the value can be
    /// locked.
    /// See `try_borrow_mut` for a non-blocking version.
    pub fn borrow_mut(&self) -> BorrowMut<T> {
        BorrowMut::new(self.0.write())
    }

    /// Attempts to mutably borrow the wrapped value.
    ///
    /// Returns an error if the value is currently mutably borrowed.
    pub fn try_borrow_mut(&self) -> Option<BorrowMut<'_, T>> {
        self.0.try_write().map(BorrowMut::new)
    }
}

/// An immutably borrowed reference to a value borrowed from a [KCell]
pub struct Borrow<'a, T: ?Sized>(MappedRwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<'a, T> {
    fn new(guard: RwLockReadGuard<'a, T>) -> Self {
        Self(RwLockReadGuard::map(guard, |x| x))
    }

    /// Makes a new Borrow for an optional component of the borrowed data.
    /// If the closure returns None then the original borrow is returned as the error.
    pub fn filter_map<U, F>(borrowed: Self, f: F) -> Result<Borrow<'a, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        MappedRwLockReadGuard::try_map(borrowed.0, f)
            .map(Borrow)
            .map_err(Borrow)
    }
}

impl<'a, T: ?Sized> Deref for Borrow<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Borrow<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A mutably borrowed reference to a value borrowed from a [KCell]
pub struct BorrowMut<'a, T: ?Sized>(MappedRwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> BorrowMut<'a, T> {
    fn new(guard: RwLockWriteGuard<'a, T>) -> Self {
        Self(RwLockWriteGuard::map(guard, |x| x))
    }

    /// Makes a new BorrowMut for an optional component of the borrowed data.
    /// If the closure returns None then the original borrow is returned as the error.
    pub fn filter_map<U, F>(borrowed: Self, f: F) -> Result<BorrowMut<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        MappedRwLockWriteGuard::try_map(borrowed.0, f)
            .map(BorrowMut)
            .map_err(BorrowMut)
    }
}

impl<'a, T: ?Sized> Deref for BorrowMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<'a, T: ?Sized> DerefMut for BorrowMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.0.deref_mut()
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for BorrowMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
