pub(crate) use std::cell::Ref as BorrowImpl;
pub(crate) use std::cell::RefCell as CellImpl;
pub(crate) use std::cell::RefMut as BorrowMutImpl;
pub(crate) use std::rc::Rc as PtrImpl;

#[doc(hidden)]
#[macro_export]
macro_rules! __make_ptr {
    ($value:expr) => {
        $crate::Ptr::from(::std::rc::Rc::new($value) as ::std::rc::Rc<_>)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __lazy {
    ($ty:ty; $expr:expr) => {{
        thread_local! {
            static VALUE: $ty = $expr.into();
        }
        VALUE.with(Clone::clone)
    }};
}

#[inline]
pub(crate) fn borrow<T: ?Sized>(cell: &CellImpl<T>) -> BorrowImpl<'_, T> {
    cell.borrow()
}

#[inline]
pub(crate) fn try_borrow<T: ?Sized>(cell: &CellImpl<T>) -> Option<BorrowImpl<'_, T>> {
    cell.try_borrow().ok()
}

#[inline]
pub(crate) fn borrow_mut<T: ?Sized>(cell: &CellImpl<T>) -> BorrowMutImpl<'_, T> {
    cell.borrow_mut()
}

#[inline]
pub(crate) fn try_borrow_mut<T: ?Sized>(cell: &CellImpl<T>) -> Option<BorrowMutImpl<'_, T>> {
    cell.try_borrow_mut().ok()
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
    BorrowImpl::filter_map(borrowed, f)
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
    BorrowMutImpl::filter_map(borrowed, f)
}
