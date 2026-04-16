use super::{KNativeFunction, KValue, decode_value, encode_value};
use crate::{
    KTuple, PluginBackend,
    Result,
    call::CallContext,
    host::{current_host_api, status_to_error, string_slice},
    vm::{abi_binary_op, abi_read_op, abi_unary_op, abi_write_op},
};
use koto_api::{
    KotoCollection, KotoIdentity, KotoIndexSwap, KotoMap, KotoMapBuilder, KotoMapLookup,
    KotoMapSource, KotoMapSourceMut, KotoMetaMap, KotoSlice, MetaKey as SharedMetaKey,
};
use koto_ffi as abi;
use std::{fmt, mem::ManuallyDrop};

/// The meta key type used by the plugin API.
pub type MetaKey = SharedMetaKey<crate::KString>;

/// A host-backed Koto map value.
///
/// This uses a single runtime-owned map handle for both ordinary map values and
/// maps being constructed during plugin initialization.
pub struct KMap {
    api: *const abi::KotoHostApiV1,
    handle: abi::KMap,
}

/// A borrowed view over map data.
pub struct KMapData<'a> {
    api: *const abi::KotoHostApiV1,
    data: abi::KMapData,
    _map: std::marker::PhantomData<&'a KMap>,
}

/// A compatibility mutable view over map data.
pub struct KMapDataMut<'a> {
    map: &'a KMap,
}

impl KMap {
    /// Creates a new map with the given `@type`.
    pub fn with_type(type_name: &str) -> Self {
        let api = current_host_api();
        Self {
            api: api as *const _,
            handle: unsafe { (api.map_new_with_type)(string_slice(type_name)) },
        }
    }

    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::Map));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.map_value },
        }
    }

    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn handle(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::Map,
            data: abi::KValueData {
                map_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::Map,
            data: abi::KValueData {
                map_value: this.handle,
            },
        }
    }

    /// Creates a map from the provided key/value entries.
    pub fn from_entries(entries: &[(KValue, KValue)]) -> Self {
        let api = current_host_api();
        let encoded = entries
            .iter()
            .cloned()
            .map(|(key, value)| abi::KotoMapEntry {
                key: encode_value(api, key),
                value: encode_value(api, value),
            })
            .collect::<Vec<_>>();
        Self {
            api: api as *const _,
            handle: unsafe { (api.map_make)(encoded.as_ptr(), encoded.len()) },
        }
    }

    /// Adds a value entry to the map using a string key.
    pub fn insert(&self, key: &str, value: impl Into<KValue>) {
        let api = self.api();
        unsafe {
            (api.map_insert_value)(
                self.handle,
                string_slice(key),
                encode_value(api, value.into()),
            )
        };
    }

    /// Adds a function entry to the map using a string key.
    pub fn add_fn(&self, key: &str, f: impl Fn(&mut CallContext) -> Result<KValue> + 'static) {
        self.insert(key, KNativeFunction::new(f));
    }

    /// Inserts a value into the map's meta map.
    pub fn insert_meta(&mut self, key: MetaKey, value: impl Into<KValue>) {
        let api = self.api();
        unsafe {
            (api.map_insert_meta_value)(
                self.handle,
                encode_meta_key(&key),
                encode_value(api, value.into()),
            )
        };
    }

    /// Adds a function to the map's meta map.
    pub fn add_meta_fn(
        &mut self,
        key: MetaKey,
        f: impl Fn(&mut CallContext) -> Result<KValue> + 'static,
    ) {
        self.insert_meta(key, KNativeFunction::new(f));
    }

    /// Swaps two entries by index.
    pub fn swap_indices(&self, a: usize, b: usize) -> Result<()> {
        let api = self.api();
        let status = unsafe { (api.map_swap_indices)(self.handle, a, b) };
        if status.code == abi::KotoStatusCode::Ok {
            Ok(())
        } else {
            Err(status_to_error(status))
        }
    }

    /// Returns the entry at `index`, if present.
    pub fn get_index(&self, index: usize) -> Option<(KValue, KValue)> {
        if index >= self.len() {
            return None;
        }

        let api = self.api();
        let key = unsafe { (api.map_key_at)(self.handle, index) };
        let value = unsafe { (api.map_value_at)(self.handle, index) };
        let result = Some((decode_value(api, key).ok()?, decode_value(api, value).ok()?));
        unsafe {
            (api.value_free)(key);
            (api.value_free)(value);
        }
        result
    }

    /// Returns true if the map contains the given meta key.
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        let api = self.api();
        match key {
            MetaKey::ReadOp(op) => unsafe {
                (api.map_contains_meta_read)(self.handle, abi_read_op(*op))
            },
            MetaKey::WriteOp(op) => unsafe {
                (api.map_contains_meta_write)(self.handle, abi_write_op(*op))
            },
            _ => false,
        }
    }

    /// Returns a cloned meta value for the given key.
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<KValue> {
        let api = self.api();
        let value = match key {
            MetaKey::ReadOp(op) => unsafe {
                (api.map_get_meta_read)(self.handle, abi_read_op(*op))
            },
            MetaKey::WriteOp(op) => unsafe {
                (api.map_get_meta_write)(self.handle, abi_write_op(*op))
            },
            _ => return None,
        };

        if matches!(value.kind, abi::KValueKind::Null) {
            None
        } else {
            let result = decode_value(api, value).ok();
            unsafe { (api.value_free)(value) };
            result
        }
    }

    /// Returns `true` if both maps refer to the same underlying runtime instance.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        let api = self.api();
        std::ptr::eq(api, other.api())
            && unsafe { (api.value_is_same_instance)(self.handle(), other.handle()) }
    }

    /// Returns a compatibility mutable view over the map's data.
    pub fn data_mut(&self) -> KMapDataMut<'_> {
        KMapDataMut { map: self }
    }

    /// Returns a borrowed view over the map's data.
    pub fn data(&self) -> KMapData<'_> {
        KMapData {
            api: self.api,
            data: unsafe { (self.api().map_data)(self.handle) },
            _map: std::marker::PhantomData,
        }
    }

    pub(crate) fn into_export_value(self) -> abi::KValue {
        self.into_raw()
    }

    pub(crate) fn display_id(&self) -> usize {
        self.handle.data as usize
    }
}

fn encode_meta_key(key: &MetaKey) -> abi::MetaKey {
    match key {
        MetaKey::UnaryOp(op) => abi::MetaKey {
            kind: abi::MetaKeyKind::UnaryOp,
            data: abi::MetaKeyData {
                unary_op: abi_unary_op(*op),
            },
        },
        MetaKey::BinaryOp(op) => abi::MetaKey {
            kind: abi::MetaKeyKind::BinaryOp,
            data: abi::MetaKeyData {
                binary_op: abi_binary_op(*op),
            },
        },
        MetaKey::ReadOp(op) => abi::MetaKey {
            kind: abi::MetaKeyKind::ReadOp,
            data: abi::MetaKeyData {
                read_op: abi_read_op(*op),
            },
        },
        MetaKey::WriteOp(op) => abi::MetaKey {
            kind: abi::MetaKeyKind::WriteOp,
            data: abi::MetaKeyData {
                write_op: abi_write_op(*op),
            },
        },
        MetaKey::Call => abi::MetaKey {
            kind: abi::MetaKeyKind::Call,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
        MetaKey::Named(name) => abi::MetaKey {
            kind: abi::MetaKeyKind::Named,
            data: abi::MetaKeyData {
                string: string_slice(name),
            },
        },
        MetaKey::Test(name) => abi::MetaKey {
            kind: abi::MetaKeyKind::Test,
            data: abi::MetaKeyData {
                string: string_slice(name),
            },
        },
        MetaKey::PreTest => abi::MetaKey {
            kind: abi::MetaKeyKind::PreTest,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
        MetaKey::PostTest => abi::MetaKey {
            kind: abi::MetaKeyKind::PostTest,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
        MetaKey::Main => abi::MetaKey {
            kind: abi::MetaKeyKind::Main,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
        MetaKey::Type => abi::MetaKey {
            kind: abi::MetaKeyKind::Type,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
        MetaKey::Base => abi::MetaKey {
            kind: abi::MetaKeyKind::Base,
            data: abi::MetaKeyData {
                unary_op: abi::UnaryOp::Debug,
            },
        },
    }
}

fn map_keys_equal(a: &KValue, b: &KValue) -> bool {
    match (a, b) {
        (KValue::Null, KValue::Null) => true,
        (KValue::Bool(a), KValue::Bool(b)) => a == b,
        (KValue::Number(a), KValue::Number(b)) => a == b,
        (KValue::Str(a), KValue::Str(b)) => a == b,
        (KValue::Tuple(a), KValue::Tuple(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(a, b)| map_keys_equal(&a, &b))
        }
        _ => false,
    }
}

impl From<Vec<(KValue, KValue)>> for KMap {
    fn from(entries: Vec<(KValue, KValue)>) -> Self {
        Self::from_entries(&entries)
    }
}

impl FromIterator<(KValue, KValue)> for KMap {
    fn from_iter<T: IntoIterator<Item = (KValue, KValue)>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl KotoCollection<PluginBackend> for KMap {
    fn len(&self) -> usize {
        unsafe { (self.api().map_len)(self.handle) }
    }
}

impl KotoCollection<PluginBackend> for KMapData<'_> {
    fn len(&self) -> usize {
        self.data.len
    }
}

impl KotoSlice<PluginBackend> for KMapData<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index).map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoMapLookup<PluginBackend> for KMapData<'_> {
    fn get_key(&self, key: &KValue) -> Option<KValue> {
        (0..self.len()).find_map(|index| {
            self.get_index(index)
                .and_then(|(entry_key, value)| map_keys_equal(&entry_key, key).then_some(value))
        })
    }
}

impl KotoMap<PluginBackend> for KMapData<'_> {
    fn get_index(&self, index: usize) -> Option<(KValue, KValue)> {
        if index >= self.data.len {
            return None;
        }

        let api = unsafe { &*self.api };
        let entry = unsafe { (api.map_data_get_entry)(self.data, index) };
        if entry.key == abi::KValueView::default() || entry.value == abi::KValueView::default() {
            return None;
        }

        let key = unsafe { (api.value_view_clone)(entry.key) };
        let value = unsafe { (api.value_view_clone)(entry.value) };
        let result = Some((decode_value(api, key).ok()?, decode_value(api, value).ok()?));
        unsafe {
            (api.value_free)(key);
            (api.value_free)(value);
        }
        result
    }
}

impl KotoCollection<PluginBackend> for KMapDataMut<'_> {
    fn len(&self) -> usize {
        self.map.len()
    }
}

impl KotoSlice<PluginBackend> for KMapDataMut<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index).map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoMapLookup<PluginBackend> for KMapDataMut<'_> {
    fn get_key(&self, key: &KValue) -> Option<KValue> {
        self.map.get_key(key)
    }
}

impl KotoMap<PluginBackend> for KMapDataMut<'_> {
    fn get_index(&self, index: usize) -> Option<(KValue, KValue)> {
        self.map.get_index(index)
    }
}

impl KotoMapSource<PluginBackend> for KMap {
    type Data<'a>
        = KMapData<'a>
    where
        Self: 'a;

    fn data(&self) -> Self::Data<'_> {
        KMap::data(self)
    }

    fn from_entries(entries: &[(KValue, KValue)]) -> Self {
        KMap::from_entries(entries)
    }
}

impl KotoSlice<PluginBackend> for KMap {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index)
            .map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoMapSourceMut<PluginBackend> for KMap {
    type DataMut<'a>
        = KMapDataMut<'a>
    where
        Self: 'a;

    fn data_mut(&self) -> Self::DataMut<'_> {
        KMapDataMut { map: self }
    }
}

impl KotoMetaMap for KMap {
    type MetaKey = MetaKey;
    type Value = KValue;

    fn contains_meta_key(&self, key: &Self::MetaKey) -> bool {
        self.contains_meta_key(key)
    }

    fn get_meta_value(&self, key: &Self::MetaKey) -> Option<Self::Value> {
        self.get_meta_value(key)
    }
}

impl KotoIdentity for KMap {
    fn is_same_instance(&self, other: &Self) -> bool {
        KMap::is_same_instance(self, other)
    }
}

impl KotoIndexSwap<PluginBackend> for KMap {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        KMap::swap_indices(self, a, b)
    }
}

impl KotoIndexSwap<PluginBackend> for KMapDataMut<'_> {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        self.map.swap_indices(a, b)
    }
}

impl KotoMapBuilder<PluginBackend> for KMap {
    type MetaKey = MetaKey;

    fn with_type(type_name: &str) -> Self {
        KMap::with_type(type_name)
    }

    fn insert(&self, key: &str, value: impl Into<KValue>) {
        self.insert(key, value);
    }

    fn add_fn<F>(&self, key: &str, f: F)
    where
        F: for<'a> Fn(&mut CallContext) -> std::result::Result<KValue, crate::Error>
            + Send
            + Sync
            + 'static,
    {
        self.add_fn(key, f);
    }

    fn insert_meta(&mut self, key: Self::MetaKey, value: impl Into<KValue>) {
        self.insert_meta(key, value);
    }

    fn add_meta_fn<F>(&mut self, key: Self::MetaKey, f: F)
    where
        F: for<'a> Fn(&mut CallContext) -> std::result::Result<KValue, crate::Error>
            + Send
            + Sync
            + 'static,
    {
        self.add_meta_fn(key, f);
    }
}

impl Clone for KMap {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
    }
}

impl Drop for KMap {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.handle()) };
    }
}

impl fmt::Debug for KMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KMap")
    }
}
