use crate::abi;
cfg_select! {
    target_arch = "wasm32" => {
        use crate::wasm_support;
        use koto_ffi::{KValueKind, wasm};
    }
    _ => {
        use crate::host::{current_host_api, string_slice};
    }
}
#[cfg(target_arch = "wasm32")]
use std::ptr;
#[cfg(target_arch = "wasm32")]
use std::sync::OnceLock;
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    mem::ManuallyDrop,
    ops::Deref,
};

/// A host-backed string type used by the derive macros and plugin helpers.
pub struct KString {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    api: *const abi::KotoHostApiV1,
    handle: abi::KString,
    #[cfg(target_arch = "wasm32")]
    cache: OnceLock<Box<str>>,
}

impl KString {
    pub(crate) fn from_handle(api: &abi::KotoHostApiV1, handle: abi::KString) -> Self {
        Self {
            api: api as *const _,
            handle,
            #[cfg(target_arch = "wasm32")]
            cache: OnceLock::new(),
        }
    }

    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_wasm_existing(handle: wasm::KValue) -> Self {
        let handle = unsafe { wasm::value_clone(handle) };
        let handle = wasm_support::wasm_value_to_native(handle);
        debug_assert!(matches!(handle.kind, KValueKind::String));
        Self {
            api: ptr::null(),
            handle: unsafe { handle.data.string_value },
            cache: OnceLock::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn from_slice(api: &abi::KotoHostApiV1, value: abi::KStringSlice) -> Self {
        Self {
            api: api as *const _,
            handle: unsafe { (api.string_make)(value) },
            #[cfg(target_arch = "wasm32")]
            cache: OnceLock::new(),
        }
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::String));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.string_value },
            #[cfg(target_arch = "wasm32")]
            cache: OnceLock::new(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn handle(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::String,
            data: abi::KValueData {
                string_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::String,
            data: abi::KValueData {
                string_value: this.handle,
            },
        }
    }

    /// Returns the string as `&str`.
    pub fn as_str(&self) -> &str {
        cfg_select! {
            target_arch = "wasm32" => {
                self.cache
                    .get_or_init(|| {
                        let slice = unsafe {
                            wasm::string_as_slice(wasm_support::native_string_to_wasm(self.handle))
                        };
                        if slice.ptr == 0 || slice.len == 0 {
                            "".into()
                        } else {
                            let bytes = unsafe {
                                std::slice::from_raw_parts(slice.ptr as *const u8, slice.len as usize)
                            };
                            let string = std::str::from_utf8(bytes)
                                .expect("host returned a non-utf8 KString")
                                .to_owned()
                                .into_boxed_str();
                            unsafe {
                                wasm::string_slice_free(slice);
                            }
                            string
                        }
                    })
                    .as_ref()
            }
            _ => {
                let slice = unsafe { (self.api().string_as_slice)(self.handle) };
                if slice.ptr.is_null() || slice.len == 0 {
                    ""
                } else {
                    let bytes = unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) };
                    std::str::from_utf8(bytes).expect("host returned a non-utf8 KString")
                }
            }
        }
    }
}

impl From<String> for KString {
    fn from(value: String) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                Self {
                    api: ptr::null(),
                    handle: wasm_support::wasm_string_to_native(unsafe {
                        wasm::string_make(wasm_support::string_slice(&value))
                    }),
                    cache: OnceLock::new(),
                }
            }
            _ => {
                let api = current_host_api();
                Self::from_slice(api, string_slice(&value))
            }
        }
    }
}

impl From<&str> for KString {
    fn from(value: &str) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                Self {
                    api: ptr::null(),
                    handle: wasm_support::wasm_string_to_native(unsafe {
                        wasm::string_make(wasm_support::string_slice(value))
                    }),
                    cache: OnceLock::new(),
                }
            }
            _ => {
                let api = current_host_api();
                Self::from_slice(api, string_slice(value))
            }
        }
    }
}

impl From<KString> for String {
    fn from(value: KString) -> Self {
        value.as_str().to_string()
    }
}

impl Deref for KString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for KString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<KString> for &str {
    fn eq(&self, other: &KString) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<str> for KString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<KString> for str {
    fn eq(&self, other: &KString) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<&str> for KString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq for KString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for KString {}

impl PartialOrd<KString> for &str {
    fn partial_cmp(&self, other: &KString) -> Option<Ordering> {
        PartialOrd::partial_cmp(*self, other.as_str())
    }
}

impl PartialOrd<&str> for KString {
    fn partial_cmp(&self, other: &&str) -> Option<Ordering> {
        PartialOrd::partial_cmp(self.as_str(), *other)
    }
}

impl PartialOrd for KString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for KString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for KString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Clone for KString {
    fn clone(&self) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                let cloned = unsafe {
                    wasm::value_clone(wasm_support::native_value_to_wasm(self.handle()))
                };
                debug_assert!(matches!(cloned.kind, KValueKind::String));
                let cloned = wasm_support::wasm_value_to_native(cloned);
                Self {
                    api: ptr::null(),
                    handle: unsafe { cloned.data.string_value },
                    cache: OnceLock::new(),
                }
            }
            _ => {
                let api = self.api();
                Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
            }
        }
    }
}

impl Drop for KString {
    fn drop(&mut self) {
        cfg_select! {
            target_arch = "wasm32" => unsafe {
                wasm::value_free(wasm_support::native_value_to_wasm(self.handle()))
            },
            _ => unsafe {
                (self.api().value_free)(self.handle())
            },
        }
    }
}
