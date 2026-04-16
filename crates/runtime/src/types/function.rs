#[cfg(feature = "plugin")]
use crate::plugin_host::transfer::AbiTransfer;
use crate::{KList, vm::NonLocals};
use koto_bytecode::{Chunk, FunctionFlags};
#[cfg(feature = "plugin")]
use koto_ffi as abi;
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
    /// Context for the function, including captures and access to non-locals
    pub context: Option<Ptr<FunctionContext>>,
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
        context: Option<Ptr<FunctionContext>>,
    ) -> Self {
        Self {
            chunk,
            ip,
            arg_count,
            optional_arg_count,
            flags,
            context,
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

    /// Provide access to the function's captures
    pub fn captures(&self) -> Option<&KList> {
        self.context
            .as_ref()
            .and_then(|context| context.captures.as_ref())
    }

    /// Provide access to the function's non-locals
    pub fn non_locals(&self) -> Option<NonLocals> {
        self.context
            .as_ref()
            .and_then(|context| context.non_locals.clone())
    }
}

#[cfg(feature = "plugin")]
impl AbiTransfer for KFunction {
    type Abi = abi::KFunction;

    unsafe fn into_abi(self) -> Self::Abi {
        abi::KFunction {
            chunk: unsafe { Ptr::into_raw(self.chunk) } as *mut _,
            ip: self.ip,
            arg_count: self.arg_count,
            optional_arg_count: self.optional_arg_count,
            flags: self.flags.into(),
            _reserved: 0,
            context: self
                .context
                .map(|context| unsafe { Ptr::into_raw(context) } as *mut _)
                .unwrap_or(std::ptr::null_mut()),
        }
    }

    unsafe fn from_abi(function: Self::Abi) -> Self {
        Self {
            chunk: unsafe { Ptr::from_raw(function.chunk as *const Chunk) },
            ip: function.ip,
            arg_count: function.arg_count,
            optional_arg_count: function.optional_arg_count,
            flags: FunctionFlags::try_from(function.flags)
                .expect("invalid function flags in plugin ABI"),
            context: (!function.context.is_null())
                .then(|| unsafe { Ptr::from_raw(function.context as *const FunctionContext) }),
            _niche: Niche::default(),
        }
    }

    unsafe fn clone_from_abi(function: Self::Abi) -> Self {
        Self {
            chunk: unsafe { Ptr::clone_from_raw(function.chunk as *const Chunk) },
            ip: function.ip,
            arg_count: function.arg_count,
            optional_arg_count: function.optional_arg_count,
            flags: FunctionFlags::try_from(function.flags)
                .expect("invalid function flags in plugin ABI"),
            context: (!function.context.is_null()).then(|| unsafe {
                Ptr::clone_from_raw(function.context as *const FunctionContext)
            }),
            _niche: Niche::default(),
        }
    }
}

pub struct FunctionContext {
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

    /// The non-locals available to the function
    pub non_locals: Option<NonLocals>,
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
