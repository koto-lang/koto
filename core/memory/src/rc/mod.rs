//! Owned pointers that wrap the standard Rc and RefCell types

mod ptr;
mod ptr_mut;

pub use ptr::*;
pub use ptr_mut::*;

/// A wrapper for comparing pointer addresses
#[derive(Copy, Clone, PartialEq)]
pub struct Address(*const u8);

impl<T: ?Sized> From<*const T> for Address {
    fn from(pointer: *const T) -> Self {
        Self(pointer as *const u8)
    }
}
