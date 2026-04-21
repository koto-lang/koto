//! Plugin authoring helpers for dynamic Koto plugins

#![warn(missing_docs)]

#[cfg(target_arch = "wasm32")]
use std::alloc::{Layout, alloc, dealloc};

mod abi;
mod call;
mod display_context;
mod error;
mod host;
mod send_sync;
mod types;
mod vm;
#[cfg(target_arch = "wasm32")]
mod wasm_support;

pub mod prelude;

#[cfg(test)]
mod tests;

#[doc(hidden)]
pub mod __private;

pub use crate::{
    call::CallContext,
    display_context::DisplayContext,
    error::{Error, Result, unexpected_args, unexpected_args_after_instance, unexpected_type},
    send_sync::{KotoSend, KotoSync},
    types::{
        Borrow, BorrowMut, IsIterable, KFunction, KIterator, KIteratorOutput, KList, KMap,
        KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KValue, KotoField, KotoObject,
        MetaKey, MethodContext, ObjectBorrow, ObjectBorrowMut, PluginBackend,
    },
    vm::KotoVm,
};
#[doc(hidden)]
pub type ValueKey = KValue;
#[doc(hidden)]
pub type ValueMap =
    indexmap::IndexMap<ValueKey, KValue, std::hash::BuildHasherDefault<crate::api::KotoHasher>>;

/// The shared API backend marker for plugins.
pub type Backend = PluginBackend;

pub use koto_api as api;
pub use koto_derive as derive;

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Allocates guest linear memory for host-driven wasm ABI calls.
pub unsafe extern "C" fn koto_alloc(size: u32, align: u32) -> u32 {
    if size == 0 {
        return 0;
    }

    let Ok(layout) = Layout::from_size_align(size as usize, align.max(1) as usize) else {
        return 0;
    };

    unsafe { alloc(layout) as usize as u32 }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Frees guest linear memory previously allocated with [`koto_alloc`].
pub unsafe extern "C" fn koto_free(ptr: u32, size: u32, align: u32) {
    if ptr == 0 || size == 0 {
        return;
    }

    let Ok(layout) = Layout::from_size_align(size as usize, align.max(1) as usize) else {
        return;
    };

    unsafe { dealloc(ptr as usize as *mut u8, layout) };
}

#[doc(hidden)]
pub fn with_host_api<T>(
    host_api: &__private::koto_ffi::native::KotoHostApiV1,
    f: impl FnOnce() -> T,
) -> T {
    host::with_host_api(host_api, f)
}

#[doc(hidden)]
cfg_select! {
    target_arch = "wasm32" => {
        /// Converts a plugin export map into the wasm ABI value representation.
        pub fn build_plugin(map: KMap) -> __private::koto_ffi::wasm::KValue {
            wasm_support::native_value_to_wasm(map.into_export_value())
        }
    }
    _ => {
        /// Converts a plugin export map into the native ABI value representation.
        pub fn build_plugin(map: KMap) -> __private::koto_ffi::native::KValue {
            map.into_export_value()
        }
    }
}

/// Creates an error using `format!` syntax.
#[macro_export]
macro_rules! runtime_error {
    ($message:literal) => {
        Err($crate::Error::from($message))
    };
    ($message:expr) => {
        Err($crate::Error::from($message))
    };
    ($message:literal, $($args:expr),+ $(,)?) => {
        Err($crate::Error::from(format!($message, $($args),+)))
    };
}

// /// A helper used by the derive macros when caching string values.
// #[macro_export]
// macro_rules! lazy {
//     ($ty:ty; $value:expr) => {
//         <$ty as ::std::convert::From<&'static str>>::from($value)
//     };
// }

/// Exports a plugin initializer with the required ABI symbol name.
#[macro_export]
macro_rules! export_plugin {
    ($make_plugin:path) => {
        cfg_select! {
            target_arch = "wasm32" => {
                #[unsafe(no_mangle)]
                pub unsafe extern "C" fn koto_plugin_init_v1(
                    out_ptr: u32,
                    status_ptr: u32,
                ) {
                    let out = unsafe {
                        &mut *(out_ptr as usize as *mut $crate::__private::koto_ffi::wasm::KValue)
                    };
                    let status = unsafe {
                        &mut *(status_ptr as usize as *mut $crate::__private::koto_ffi::wasm::KotoStatus)
                    };

                    *out = $crate::__private::koto_ffi::wasm::KValue::null();
                    *status = match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                        let map = $make_plugin();
                        *out = $crate::build_plugin(map);
                        $crate::__private::koto_ffi::wasm::KotoStatus::ok()
                    })) {
                        Ok(status) => status,
                        Err(_) => $crate::__private::koto_ffi::wasm::KotoStatus {
                            code: $crate::__private::koto_ffi::KotoStatusCode::Error,
                            error: 0,
                            is_unimplemented: false,
                            message: $crate::__private::koto_ffi::wasm::KStringSlice { ptr: 0, len: 0 },
                        },
                    };
                }
            }
            _ => {
                #[unsafe(no_mangle)]
                pub unsafe extern "C" fn koto_plugin_init_v1(
                    host_api: *const $crate::__private::koto_ffi::native::KotoHostApiV1,
                    out: *mut $crate::__private::koto_ffi::native::KValue,
                ) -> $crate::__private::koto_ffi::native::KotoStatus {
                    match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                        let host_api = unsafe { &*host_api };
                        let map = $crate::with_host_api(host_api, || $make_plugin());
                        unsafe { *out = $crate::build_plugin(map) };
                        $crate::__private::koto_ffi::native::KotoStatus::ok()
                    })) {
                        Ok(status) => status,
                        Err(_) => $crate::Error::new("plugin initialization panicked").into_status(),
                    }
                }
            }
        }
    };
}
