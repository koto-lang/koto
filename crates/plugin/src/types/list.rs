use super::{KValue, decode_value, encode_value};
use crate::{
    PluginBackend,
    Result,
    host::{current_host_api, status_to_error},
};
use koto_api::{
    KotoCollection, KotoIdentity, KotoIndexSwap, KotoSequence, KotoSequenceMut, KotoSlice,
    KotoSliceMut,
};
use koto_ffi as abi;
use std::{fmt, marker::PhantomData, mem::ManuallyDrop};

/// A host-backed Koto list.
pub struct KList {
    api: *const abi::KotoHostApiV1,
    handle: abi::KList,
}

/// A borrowed view over plugin list data.
#[derive(Clone, Copy)]
pub struct KListData<'a> {
    api: *const abi::KotoHostApiV1,
    slice: abi::KValueSlice,
    _list: PhantomData<&'a KList>,
}

/// A compatibility mutable view over list data.
pub struct KListDataMut<'a> {
    list: &'a KList,
}

impl KList {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::List));
        Self {
            api: api as *const _,
            handle: abi::KList(unsafe { handle.data.handle }),
        }
    }

    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn as_value(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::List,
            data: abi::KValueData {
                handle: self.handle.0,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::List,
            data: abi::KValueData {
                handle: this.handle.0,
            },
        }
    }

    pub(crate) fn display_id(&self) -> usize {
        self.handle.0 as usize
    }

    /// Creates a list from the provided slice.
    pub fn from_slice(values: &[KValue]) -> Self {
        let api = current_host_api();
        let encoded = values
            .iter()
            .cloned()
            .map(|value| encode_value(api, value))
            .collect::<Vec<_>>();
        Self {
            api: api as *const _,
            handle: unsafe { (api.list_make)(encoded.as_ptr(), encoded.len()) },
        }
    }

    /// Replaces a list item at the given index.
    pub fn set(&self, index: usize, value: KValue) -> Result<()> {
        let api = self.api();
        let value = encode_value(api, value);
        let status = unsafe { (api.list_set)(self.handle, index, value) };
        if status.code == abi::KotoStatusCode::Ok {
            Ok(())
        } else {
            Err(status_to_error(status))
        }
    }

    /// Swaps two list entries.
    pub fn swap_indices(&self, a: usize, b: usize) -> Result<()> {
        if a == b {
            return Ok(());
        }

        let a_value = self
            .get(a)
            .ok_or_else(|| crate::Error::new(format!("invalid list index ({a})")))?;
        let b_value = self
            .get(b)
            .ok_or_else(|| crate::Error::new(format!("invalid list index ({b})")))?;
        self.set(a, b_value)?;
        self.set(b, a_value)
    }

    /// Returns a compatibility mutable view over the list's data.
    pub fn data_mut(&self) -> KListDataMut<'_> {
        KListDataMut { list: self }
    }

    /// Returns a borrowed view over the list's data.
    pub fn data(&self) -> KListData<'_> {
        KListData {
            api: self.api,
            slice: unsafe { (self.api().list_data)(self.handle) },
            _list: PhantomData,
        }
    }

    /// Returns `true` if both lists refer to the same underlying runtime instance.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        let api = self.api();
        std::ptr::eq(api, other.api())
            && unsafe { (api.value_is_same_instance)(self.as_value(), other.as_value()) }
    }
}

impl From<Vec<KValue>> for KList {
    fn from(values: Vec<KValue>) -> Self {
        Self::from_slice(&values)
    }
}

impl FromIterator<KValue> for KList {
    fn from_iter<T: IntoIterator<Item = KValue>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl KotoCollection<PluginBackend> for KList {
    fn len(&self) -> usize {
        unsafe { (self.api().list_len)(self.handle) }
    }
}

impl KotoCollection<PluginBackend> for KListData<'_> {
    fn len(&self) -> usize {
        self.slice.len
    }
}

impl KotoSlice<PluginBackend> for KListData<'_> {
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

impl KotoCollection<PluginBackend> for KListDataMut<'_> {
    fn len(&self) -> usize {
        self.list.len()
    }
}

impl KotoSlice<PluginBackend> for KListDataMut<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.list.get(index)
    }
}

impl KotoSliceMut<PluginBackend> for KListDataMut<'_> {
    fn set(&mut self, index: usize, value: KValue) -> std::result::Result<(), crate::Error> {
        self.list.set(index, value)
    }
}

impl KotoIndexSwap<PluginBackend> for KListDataMut<'_> {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        self.list.swap_indices(a, b)
    }
}

impl KotoSlice<PluginBackend> for KList {
    fn get(&self, index: usize) -> Option<KValue> {
        KList::data(self).get(index)
    }
}

impl KotoSequence<PluginBackend> for KList {
    type Data<'a>
        = KListData<'a>
    where
        Self: 'a;

    fn data(&self) -> Self::Data<'_> {
        KList::data(self)
    }

    fn from_slice(values: &[KValue]) -> Self {
        let api = current_host_api();
        let encoded = values
            .iter()
            .cloned()
            .map(|value| encode_value(api, value))
            .collect::<Vec<_>>();
        KList {
            api: api as *const _,
            handle: unsafe { (api.list_make)(encoded.as_ptr(), encoded.len()) },
        }
    }
}

impl KotoSequenceMut<PluginBackend> for KList {
    type DataMut<'a>
        = KListDataMut<'a>
    where
        Self: 'a;

    fn data_mut(&self) -> Self::DataMut<'_> {
        KListDataMut { list: self }
    }
}

impl KotoIdentity for KList {
    fn is_same_instance(&self, other: &Self) -> bool {
        KList::is_same_instance(self, other)
    }
}

impl KotoIndexSwap<PluginBackend> for KList {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        KList::swap_indices(self, a, b)
    }
}

impl Clone for KList {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.as_value()) })
    }
}

impl Drop for KList {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.as_value()) };
    }
}

impl fmt::Debug for KList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KList")
    }
}
