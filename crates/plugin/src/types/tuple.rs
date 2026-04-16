use super::{KValue, decode_value, encode_value};
use crate::{PluginBackend, host::current_host_api};
use koto_api::{KotoCollection, KotoIdentity, KotoSequence, KotoSlice};
use koto_ffi as abi;
use std::{fmt, marker::PhantomData, mem::ManuallyDrop};

/// A host-backed Koto tuple.
pub struct KTuple {
    api: *const abi::KotoHostApiV1,
    handle: abi::KTuple,
}

/// A borrowed view over tuple data.
#[derive(Clone, Copy)]
pub struct KTupleData<'a> {
    api: *const abi::KotoHostApiV1,
    slice: abi::KValueSlice,
    _tuple: PhantomData<&'a KTuple>,
}

impl KTuple {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
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
        unsafe { (self.api().tuple_len)(self.handle) }
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
            slice: unsafe { (self.api().tuple_data)(self.handle) },
            _tuple: PhantomData,
        }
    }

    fn from_slice(values: &[KValue]) -> Self {
        let api = current_host_api();
        let encoded = values
            .iter()
            .cloned()
            .map(|value| encode_value(api, value))
            .collect::<Vec<_>>();
        KTuple {
            api: api as *const _,
            handle: unsafe { (api.tuple_make)(encoded.as_ptr(), encoded.len()) },
        }
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
        let api = unsafe { &*self.api };
        let value = unsafe { (api.value_view_clone)(abi::KValueView(ptr)) };
        let result = decode_value(api, value).ok();
        unsafe { (api.value_free)(value) };
        result
    }
}

impl KotoIdentity for KTuple {
    fn is_same_instance(&self, other: &Self) -> bool {
        KTuple::is_same_instance(self, other)
    }
}

impl Clone for KTuple {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.as_value()) })
    }
}

impl Drop for KTuple {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.as_value()) };
    }
}

impl fmt::Debug for KTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KTuple")
    }
}
