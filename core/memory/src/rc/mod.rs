//! Owned pointers that wrap the standard Rc and RefCell types

mod ptr;
mod ptr_mut;

pub use {ptr::*, ptr_mut::*};
