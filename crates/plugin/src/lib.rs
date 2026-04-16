//! Plugin authoring helpers for dynamic Koto plugins

#![warn(missing_docs)]

mod call;
mod display_context;
mod error;
mod host;
mod send_sync;
mod types;
mod vm;

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

#[doc(hidden)]
pub fn with_host_api<T>(host_api: &__private::koto_ffi::KotoHostApiV1, f: impl FnOnce() -> T) -> T {
    host::with_host_api(host_api, f)
}

#[doc(hidden)]
pub fn build_plugin(map: KMap) -> __private::koto_ffi::KValue {
    map.into_export_value()
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
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn koto_plugin_init_v1(
            host_api: *const $crate::__private::koto_ffi::KotoHostApiV1,
            out: *mut $crate::__private::koto_ffi::KValue,
        ) -> $crate::__private::koto_ffi::KotoStatus {
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let host_api = unsafe { &*host_api };
                let map = $crate::with_host_api(host_api, || $make_plugin());
                unsafe { *out = $crate::build_plugin(map) };
                $crate::__private::koto_ffi::KotoStatus::ok()
            })) {
                Ok(status) => status,
                Err(_) => $crate::Error::new("plugin initialization panicked").into_status(),
            }
        }
    };
}
