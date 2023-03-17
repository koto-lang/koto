use std::{
    cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut},
    fmt,
    ops::{Deref, DerefMut},
    rc::Rc,
};

/// A mutable pointer to a value in allocated memory
#[derive(Debug, Default)]
pub struct PtrMut<T: ?Sized>(Rc<RefCell<T>>);

impl<T> PtrMut<T> {
    /// Moves the value into newly allocated memory
    pub fn new(value: T) -> Self {
        Self(Rc::new(RefCell::new(value)))
    }
}

impl<T: ?Sized> PtrMut<T> {
    /// Immutably borrows the wrapped value.
    ///
    /// Multiple immutable borrows can be made at the same time.
    ///
    /// If the value is currently mutably borrowed then this function will panic.
    /// See `try_borrow` for a non-panicking version.
    pub fn borrow(&self) -> Borrow<T> {
        Borrow(self.0.borrow())
    }

    /// Attempts to mutably borrow the wrapped value.
    ///
    /// Returns an error if the value is currently mutably borrowed.
    pub fn try_borrow(&self) -> Result<Borrow<'_, T>, BorrowError> {
        self.0.try_borrow().map(Borrow)
    }

    /// Mutably borrows the wrapped value.
    ///
    /// If the value is currently borrowed then this function will panic.
    /// See `try_borrow_mut` for a non-panicking version.
    pub fn borrow_mut(&self) -> BorrowMut<T> {
        BorrowMut(self.0.borrow_mut())
    }

    /// Attempts to mutably borrow the wrapped value.
    ///
    /// Returns an error if the value is currently mutably borrowed.
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<'_, T>, BorrowMutError> {
        self.0.try_borrow_mut().map(BorrowMut)
    }
}

impl<T> From<T> for PtrMut<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> From<Rc<RefCell<T>>> for PtrMut<T> {
    fn from(inner: Rc<RefCell<T>>) -> Self {
        Self(inner)
    }
}

impl<T: ?Sized> Clone for PtrMut<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

/// An immutably borrowed reference to a value borrowed from a [PtrMut]
pub struct Borrow<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Borrow<'a, T> {
    /// Makes a new Borrow for an optional component of the borrowed data.
    /// If the closure returns None then the original borrow is returned as the error.
    pub fn filter_map<U, F>(borrowed: Self, f: F) -> Result<Borrow<'a, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        Ref::filter_map(borrowed.0, f).map(Borrow).map_err(Borrow)
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

/// A mutably borrowed reference to a value borrowed from a [PtrMut]
pub struct BorrowMut<'a, T: ?Sized>(RefMut<'a, T>);

impl<'a, T: ?Sized> BorrowMut<'a, T> {
    /// Makes a new BorrowMut for an optional component of the borrowed data.
    /// If the closure returns None then the original borrow is returned as the error.
    pub fn filter_map<U, F>(borrowed: Self, f: F) -> Result<BorrowMut<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        RefMut::filter_map(borrowed.0, f)
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
