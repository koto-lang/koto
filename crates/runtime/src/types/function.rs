use crate::KList;
use koto_bytecode::{Chunk, FunctionFlags};
use koto_memory::Ptr;

/// A Koto function
///
/// See also:
/// * [`KNativeFunction`](crate::KNativeFunction)
/// * [`KValue::Function`](crate::KValue::Function)
#[derive(Clone)]
pub struct KFunction {
    /// The [Chunk] in which the function can be found.
    pub chunk: Ptr<Chunk>,
    /// The start ip of the function.
    pub ip: u32,
    /// The number of arguments expected by the function
    pub arg_count: u8,
    /// The number of arguments that have default values
    pub optional_arg_count: u8,
    /// Flags that define various properties of the function
    pub flags: FunctionFlags,
    /// The optional list of captures that should be copied into scope when the function is called.
    ///
    /// The captures list starts with any default argument values, followed by values captured from
    /// parent scopes.
    //
    // Q. Why use a KList?
    // A. Because capturing values currently works by assigning by index, after the function
    //    itself has been created, and the captured function and the assigned function both need to
    //    share the same captures list. Currently the only way for this to work is to allow mutation
    //    of the shared list after the creation of the function, so a KList is a reasonable choice.
    pub captures: Option<KList>,
    // Pads the size of KFunction to exactly 24 bytes on 64 byte targets,
    // allowing KFunction to be used in niche optimization for KValue.
    _niche: Niche,
}

impl KFunction {
    /// Returns a [KFunction] with the given arguments
    pub fn new(
        chunk: Ptr<Chunk>,
        ip: u32,
        arg_count: u8,
        optional_arg_count: u8,
        flags: FunctionFlags,
        captures: Option<KList>,
    ) -> Self {
        Self {
            chunk,
            ip,
            arg_count,
            optional_arg_count,
            flags,
            captures,
            _niche: Niche::default(),
        }
    }

    /// Returns the required minimum number of arguments when calling this function
    ///
    /// This is equivalent to `self.arg_count - 1` if the function is variadic,
    /// otherwise it's equivalent to `self.arg_count`.
    pub fn expected_arg_count(&self) -> u8 {
        if self.flags.is_variadic() {
            debug_assert!(self.arg_count > 0);
            self.arg_count - 1
        } else {
            self.arg_count
        }
    }
}

// A dummy value usable in niche optimization
//
// KFunction is the only KValue variant larger than 16 bytes,
// and must be exactly 24 bytes for the compiler to find potential niches to use
// for KValue. Padding bytes aren't allowed to be used for niche optimization,
// so it's necessary to pad out KFunction with an optimizable value.
#[derive(Clone, Default)]
#[repr(u8)]
enum Niche {
    #[default]
    Value = 0,
}
