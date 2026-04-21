use crate::abi;
use crate::{
    KValue, KotoVm, Result,
    call::CallContext,
    error::Error,
    host::with_host_api,
    types::{decode_value, encode_value},
};
use std::{
    ffi::c_void,
    fmt,
    mem::ManuallyDrop,
    panic::{AssertUnwindSafe, catch_unwind},
    slice,
};
cfg_select! {
    target_arch = "wasm32" => {
        use crate::types::decode_wasm_value;
        use crate::wasm_support;
        use koto_ffi::wasm;
        use std::{
            cell::{Cell, RefCell},
            collections::HashMap,
        };
    }
    _ => {
        use crate::host::current_host_api;
    }
}

type PluginFunction = dyn Fn(&mut CallContext) -> Result<KValue> + 'static;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct FunctionWrapper {
    function: Box<PluginFunction>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    // Registered wasm native functions are removed again via `koto_plugin_native_function_drop_v1`
    // when the host releases the last handle to the guest function.
    static WASM_FUNCTIONS: RefCell<HashMap<u32, Box<PluginFunction>>> = RefCell::new(HashMap::new());
    static NEXT_WASM_FUNCTION_ID: Cell<u32> = const { Cell::new(1) };
}

#[cfg(target_arch = "wasm32")]
fn register_wasm_function(function: Box<PluginFunction>) -> u32 {
    NEXT_WASM_FUNCTION_ID.with(|next_id| {
        let id = next_id.get();
        next_id.set(id + 1);
        WASM_FUNCTIONS.with(|functions| {
            functions.borrow_mut().insert(id, function);
        });
        id
    })
}

#[cfg(target_arch = "wasm32")]
fn unregister_wasm_function(id: u32) {
    WASM_FUNCTIONS.with(|functions| {
        functions.borrow_mut().remove(&id);
    });
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
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
        cfg_select! {
            target_arch = "wasm32" => {
                let id = register_wasm_function(Box::new(function));
                let (status, out) = unsafe {
                    wasm::native_function_make(
                        wasm_support::string_slice("koto_plugin_native_function_trampoline_v1"),
                        id,
                    )
                };
                if status.code != koto_ffi::KotoStatusCode::Ok {
                    panic!("{}", wasm_support::status_to_error(status));
                }

                let out = wasm_support::wasm_value_to_native(out);
                debug_assert!(matches!(out.kind, abi::KValueKind::NativeFunction));
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { out.data.native_function_value },
                }
            }
            _ => {
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

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
        cfg_select! {
            target_arch = "wasm32" => {
                let cloned = unsafe {
                    wasm::value_clone(wasm_support::native_value_to_wasm(self.handle()))
                };
                let cloned = wasm_support::wasm_value_to_native(cloned);
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { cloned.data.native_function_value },
                }
            }
            _ => {
                let api = self.api();
                Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
            }
        }
    }
}

impl Drop for KNativeFunction {
    fn drop(&mut self) {
        cfg_select! {
            target_arch = "wasm32" => unsafe {
                wasm::value_free(wasm_support::native_value_to_wasm(self.handle()));
            },
            _ => unsafe {
                (self.api().value_free)(self.handle())
            },
        }
    }
}

impl fmt::Debug for KNativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KNativeFunction")
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
unsafe extern "C" fn drop_trampoline(user_data: *mut c_void) {
    let _ = unsafe { Box::from_raw(user_data as *mut FunctionWrapper) };
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_native_function_trampoline_v1")]
pub unsafe extern "C" fn wasm_function_trampoline(
    ctx_ptr: wasm::CallContextPtr,
    user_data: u32,
    out_ptr: wasm::KValuePtr,
    status_ptr: wasm::KotoStatusPtr,
) {
    let out = unsafe { &mut *(out_ptr as usize as *mut wasm::KValue) };
    let status = unsafe { &mut *(status_ptr as usize as *mut wasm::KotoStatus) };
    let ctx = unsafe { &*(ctx_ptr as usize as *const wasm::CallContext) };

    *out = wasm::KValue::null();
    *status = match catch_unwind(AssertUnwindSafe(|| {
        let instance = decode_wasm_value(ctx.instance).unwrap_or(KValue::Null);
        let mut call_ctx = CallContext::from_wasm(
            instance,
            ctx.args_ptr as *const wasm::KValue,
            ctx.arg_count as usize,
        );

        WASM_FUNCTIONS.with(|functions| match functions.borrow().get(&user_data) {
            Some(function) => match function(&mut call_ctx) {
                Ok(value) => {
                    *out = super::map::encode_export_value(value);
                    wasm::KotoStatus::ok()
                }
                Err(error) => wasm_support::error_status(&error.to_string()),
            },
            None => wasm_support::error_status("unknown wasm plugin function id"),
        })
    })) {
        Ok(status) => status,
        Err(_) => wasm_support::error_status("wasm plugin callback panicked"),
    };
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_native_function_drop_v1")]
pub unsafe extern "C" fn wasm_function_drop_trampoline(user_data: u32) {
    unregister_wasm_function(user_data);
}
