use super::KValue;
use crate::PluginBackend;
use crate::abi;
use koto_api::{KotoCollection, KotoIdentity, KotoSequence, KotoSlice};
cfg_select! {
    target_arch = "wasm32" => {
        use super::decode_wasm_value;
        use crate::wasm_support;
        use koto_ffi::wasm;
    }
    _ => {
        use super::{decode_value, encode_value};
        use crate::host::current_host_api;
    }
}
use std::{fmt, marker::PhantomData, mem::ManuallyDrop};

/// A host-backed Koto tuple.
pub struct KTuple {
    api: *const abi::KotoHostApiV1,
    handle: abi::KTuple,
}

/// A borrowed view over tuple data.
#[derive(Clone)]
pub struct KTupleData<'a> {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    api: *const abi::KotoHostApiV1,
    slice: abi::KValueSlice,
    _tuple: PhantomData<&'a KTuple>,
}

impl KTuple {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_wasm_existing(handle: wasm::KValue) -> Self {
        let handle = unsafe { wasm::value_clone(handle) };
        let handle = wasm_support::wasm_value_to_native(handle);
        debug_assert!(matches!(handle.kind, abi::KValueKind::Tuple));
        Self {
            api: std::ptr::null(),
            handle: unsafe { handle.data.tuple_value },
        }
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::Tuple));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.tuple_value },
        }
    }

    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn as_value(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::Tuple,
            data: abi::KValueData {
                tuple_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::Tuple,
            data: abi::KValueData {
                tuple_value: this.handle,
            },
        }
    }

    pub(crate) fn display_id(&self) -> usize {
        match self.handle.kind {
            abi::KTupleKind::Full => unsafe { self.handle.data.full as usize },
            abi::KTupleKind::Slice16 => unsafe { self.handle.data.slice16.data as usize },
            abi::KTupleKind::Slice => unsafe { self.handle.data.slice.data as usize },
        }
    }

    /// Creates a tuple from the provided slice.
    pub fn from_slice(values: &[KValue]) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                let encoded = values
                    .iter()
                    .cloned()
                    .map(super::map::encode_export_value)
                    .collect::<Vec<_>>();
                Self {
                    api: std::ptr::null(),
                    handle: wasm_support::wasm_tuple_to_native(unsafe {
                        wasm::tuple_make(encoded.as_ptr(), encoded.len() as u32)
                    }),
                }
            }
            _ => {
                let api = current_host_api();
                let encoded = values
                    .iter()
                    .cloned()
                    .map(|value| encode_value(api, value))
                    .collect::<Vec<_>>();
                Self {
                    api: api as *const _,
                    handle: unsafe { (api.tuple_make)(encoded.as_ptr(), encoded.len()) },
                }
            }
        }
    }

    /// Returns `true` if both tuples refer to the same underlying runtime instance.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        let api = self.api();
        std::ptr::eq(api, other.api())
            && unsafe { (api.value_is_same_instance)(self.as_value(), other.as_value()) }
    }
}

impl From<Vec<KValue>> for KTuple {
    fn from(values: Vec<KValue>) -> Self {
        Self::from_slice(&values)
    }
}

impl From<&[KValue]> for KTuple {
    fn from(values: &[KValue]) -> Self {
        Self::from_slice(values)
    }
}

impl<const N: usize> From<&[KValue; N]> for KTuple {
    fn from(values: &[KValue; N]) -> Self {
        Self::from_slice(values.as_slice())
    }
}

impl KotoCollection<PluginBackend> for KTuple {
    fn len(&self) -> usize {
        cfg_select! {
            target_arch = "wasm32" => {
                unsafe { wasm::tuple_len(wasm_support::native_tuple_to_wasm(self.handle)) as usize }
            }
            _ => {
                unsafe { (self.api().tuple_len)(self.handle) }
            }
        }
    }
}
impl KotoSlice<PluginBackend> for KTuple {
    fn get(&self, index: usize) -> Option<KValue> {
        self.data().get(index)
    }
}

impl KotoSequence<PluginBackend> for KTuple {
    type Data<'a>
        = KTupleData<'a>
    where
        Self: 'a;

    fn data(&self) -> Self::Data<'_> {
        KTupleData {
            api: self.api,
            slice: cfg_select! {
                target_arch = "wasm32" => {
                    wasm_support::wasm_value_slice_to_native(unsafe {
                        wasm::tuple_data(wasm_support::native_tuple_to_wasm(self.handle))
                    })
                }
                _ => {
                    unsafe { (self.api().tuple_data)(self.handle) }
                }
            },
            _tuple: PhantomData,
        }
    }

    fn from_slice(values: &[KValue]) -> Self {
        KTuple::from_slice(values)
    }
}

impl KotoCollection<PluginBackend> for KTupleData<'_> {
    fn len(&self) -> usize {
        self.slice.len
    }
}

impl KotoSlice<PluginBackend> for KTupleData<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        if index >= self.slice.len || self.slice.data.is_null() {
            return None;
        }

        let offset = index.checked_mul(self.slice.stride)?;
        let ptr = unsafe { (self.slice.data as *const u8).add(offset) }.cast();
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = self.api;
                let value = wasm_support::wasm_value_to_native(unsafe {
                    wasm::value_view_clone(
                        wasm_support::native_value_view_to_wasm(abi::KValueView(ptr))
                    )
                });
                let result = decode_wasm_value(wasm_support::native_value_to_wasm(value)).ok();
                unsafe {
                    wasm::value_free(wasm_support::native_value_to_wasm(value));
                }
                result
            }
            _ => {
                let api = unsafe { &*self.api };
                let value = unsafe { (api.value_view_clone)(abi::KValueView(ptr)) };
                let result = decode_value(api, value).ok();
                unsafe {
                    (api.value_free)(value)
                };
                result
            }
        }
    }
}

impl Drop for KTupleData<'_> {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        unsafe {
            wasm::value_slice_free(wasm_support::native_value_slice_to_wasm(self.slice));
        }
    }
}

impl KotoIdentity for KTuple {
    fn is_same_instance(&self, other: &Self) -> bool {
        KTuple::is_same_instance(self, other)
    }
}

impl Clone for KTuple {
    fn clone(&self) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                let cloned = unsafe {
                    wasm::value_clone(wasm_support::native_value_to_wasm(self.as_value()))
                };
                let cloned = wasm_support::wasm_value_to_native(cloned);
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { cloned.data.tuple_value },
                }
            }
            _ => {
                let api = self.api();
                Self::from_raw(api, unsafe { (api.value_clone)(self.as_value()) })
            }
        }
    }
}

impl Drop for KTuple {
    fn drop(&mut self) {
        cfg_select! {
            target_arch = "wasm32" => unsafe {
                wasm::value_free(wasm_support::native_value_to_wasm(self.as_value()));
            },
            _ => unsafe {
                (self.api().value_free)(self.as_value())
            },
        }
    }
}

impl fmt::Debug for KTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KTuple")
    }
}
