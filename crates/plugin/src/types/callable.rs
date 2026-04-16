use koto_ffi as abi;
use std::{
    ffi::c_void,
    fmt,
    mem::ManuallyDrop,
    panic::{AssertUnwindSafe, catch_unwind},
    slice,
};

use crate::{
    KValue, KotoVm, Result,
    call::CallContext,
    error::Error,
    host::{current_host_api, with_host_api},
    types::{decode_value, encode_value},
};

type PluginFunction = dyn Fn(&mut CallContext) -> Result<KValue> + 'static;

struct FunctionWrapper {
    function: Box<PluginFunction>,
}

/// A host-backed Koto function value.
pub struct KFunction {
    api: *const abi::KotoHostApiV1,
    handle: abi::KFunction,
}

impl KFunction {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::Function));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.function_value },
        }
    }

    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn handle(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::Function,
            data: abi::KValueData {
                function_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::Function,
            data: abi::KValueData {
                function_value: this.handle,
            },
        }
    }

    /// Returns `true` if the function is a generator.
    pub fn is_generator(&self) -> bool {
        const GENERATOR_FLAG: u8 = 1 << 1;
        self.handle.flags & GENERATOR_FLAG != 0
    }
}

impl Clone for KFunction {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
    }
}

impl Drop for KFunction {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.handle()) };
    }
}

impl fmt::Debug for KFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KFunction")
    }
}

/// A host-backed native function value.
pub struct KNativeFunction {
    api: *const abi::KotoHostApiV1,
    handle: abi::OpaqueHandle,
}

// Safety: `KNativeFunction` is an opaque host-owned handle. Cloning and dropping are routed
// through the host API, and there is no direct access to the underlying callable state here.
unsafe impl Send for KNativeFunction {}
// Safety: sharing the handle across threads is sound for the same reason; interaction remains at
// the host boundary and the handle itself has no interior Rust references.
unsafe impl Sync for KNativeFunction {}

impl KNativeFunction {
    #[doc(hidden)]
    pub fn new(function: impl Fn(&mut CallContext) -> Result<KValue> + 'static) -> Self {
        let api = current_host_api();
        let wrapper = Box::new(FunctionWrapper {
            function: Box::new(function),
        });
        let handle = unsafe {
            (api.native_function_make)(
                function_trampoline,
                Box::into_raw(wrapper).cast::<c_void>(),
                drop_trampoline,
            )
        };

        Self {
            api: api as *const _,
            handle,
        }
    }

    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::NativeFunction));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.native_function_value },
        }
    }

    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn handle(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::NativeFunction,
            data: abi::KValueData {
                native_function_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::NativeFunction,
            data: abi::KValueData {
                native_function_value: this.handle,
            },
        }
    }
}

impl Clone for KNativeFunction {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
    }
}

impl Drop for KNativeFunction {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.handle()) };
    }
}

impl fmt::Debug for KNativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KNativeFunction")
    }
}

unsafe extern "C" fn function_trampoline(
    host_api: *const abi::KotoHostApiV1,
    ctx: abi::CallContext,
    user_data: *mut c_void,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let wrapper = unsafe { &mut *(user_data as *mut FunctionWrapper) };
            let arg_handles = unsafe { slice::from_raw_parts(ctx.args, ctx.arg_count) };
            let instance = if matches!(ctx.instance.kind, abi::KValueKind::Null) {
                KValue::Null
            } else {
                decode_value(api, ctx.instance).unwrap_or(KValue::Null)
            };

            for arg in arg_handles {
                if matches!(arg.kind, abi::KValueKind::Unsupported) {
                    return Error::new("unsupported runtime value for plugin ABI v1").into_status();
                }
            }

            let mut ctx = CallContext::from_abi(
                api,
                KotoVm::from_api(api),
                instance,
                ctx.args,
                ctx.arg_count,
            );
            match (wrapper.function)(&mut ctx) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin callback panicked").into_status(),
    }
}

unsafe extern "C" fn drop_trampoline(user_data: *mut c_void) {
    let _ = unsafe { Box::from_raw(user_data as *mut FunctionWrapper) };
}
