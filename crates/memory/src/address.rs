use std::hash::{Hash, Hasher};

/// A wrapper for comparing and hashing pointer addresses
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Address(*const u8);

impl<T: ?Sized> From<*const T> for Address {
    fn from(pointer: *const T) -> Self {
        Self(pointer as *const u8)
    }
}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.0 as *const () as usize);
    }
}
