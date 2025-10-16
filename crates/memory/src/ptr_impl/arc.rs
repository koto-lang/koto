pub(crate) use parking_lot::MappedRwLockReadGuard as BorrowImpl;
pub(crate) use parking_lot::MappedRwLockWriteGuard as BorrowMutImpl;
pub(crate) use parking_lot::RwLock as CellImpl;
pub(crate) use std::sync::Arc as PtrImpl;

#[doc(hidden)]
#[macro_export]
macro_rules! __make_ptr {
    ($value:expr) => {
        $crate::Ptr::from(::std::sync::Arc::new($value) as ::std::sync::Arc<_>)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __lazy {
    ($ty:ty; $expr:expr) => {{
        static VALUE: ::std::sync::LazyLock<$ty> = ::std::sync::LazyLock::new(|| $expr.into());
        ::std::sync::LazyLock::force(&VALUE).clone()
    }};
}

#[inline]
pub(crate) fn borrow<T: ?Sized>(cell: &CellImpl<T>) -> BorrowImpl<'_, T> {
    parking_lot::RwLockReadGuard::map(cell.read(), |x| x)
}

#[inline]
pub(crate) fn try_borrow<T: ?Sized>(cell: &CellImpl<T>) -> Option<BorrowImpl<'_, T>> {
    cell.try_read()
        .map(|g| parking_lot::RwLockReadGuard::map(g, |x| x))
}

#[inline]
pub(crate) fn borrow_mut<T: ?Sized>(cell: &CellImpl<T>) -> BorrowMutImpl<'_, T> {
    parking_lot::RwLockWriteGuard::map(cell.write(), |x| x)
}

#[inline]
pub(crate) fn try_borrow_mut<T: ?Sized>(cell: &CellImpl<T>) -> Option<BorrowMutImpl<'_, T>> {
    cell.try_write()
        .map(|g| parking_lot::RwLockWriteGuard::map(g, |x| x))
}

#[inline]
pub(crate) fn borrowed_filter_map<'a, T: ?Sized, U, F>(
    borrowed: BorrowImpl<'a, T>,
    f: F,
) -> Result<BorrowImpl<'a, U>, BorrowImpl<'a, T>>
where
    F: FnOnce(&T) -> Option<&U>,
    U: ?Sized,
{
    BorrowImpl::try_map(borrowed, f)
}

#[inline]
pub(crate) fn borrowed_mut_filter_map<'a, T: ?Sized, U, F>(
    borrowed: BorrowMutImpl<'a, T>,
    f: F,
) -> Result<BorrowMutImpl<'a, U>, BorrowMutImpl<'a, T>>
where
    F: FnOnce(&mut T) -> Option<&mut U>,
    U: ?Sized,
{
    BorrowMutImpl::try_map(borrowed, f)
}
