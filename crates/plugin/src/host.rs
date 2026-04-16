use crate::Error;
use koto_ffi as abi;
use std::{cell::Cell, ptr};

thread_local! {
    static CURRENT_HOST_API: Cell<*const abi::KotoHostApiV1> = const { Cell::new(ptr::null()) };
}

pub(crate) fn with_host_api<T>(host_api: &abi::KotoHostApiV1, f: impl FnOnce() -> T) -> T {
    CURRENT_HOST_API.with(|current| {
        let previous = current.replace(host_api as *const _);
        let result = f();
        current.set(previous);
        result
    })
}

pub(crate) fn current_host_api() -> &'static abi::KotoHostApiV1 {
    CURRENT_HOST_API.with(|current| {
        let host_api = current.get();
        assert!(
            !host_api.is_null(),
            "koto_plugin helpers can only be used while a plugin callback is being executed"
        );
        unsafe { &*host_api }
    })
}

pub(crate) fn string_slice(s: &str) -> abi::KStringSlice {
    abi::KStringSlice {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

pub(crate) fn status_to_error(status: abi::KotoStatus) -> Error {
    Error::from_status(status)
}
