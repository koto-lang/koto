#[cfg(feature = "plugin")]
use crate::KCell;
#[cfg(feature = "plugin")]
use crate::plugin_host::transfer::AbiTransfer;
use crate::{Borrow, BorrowMut, Error, PtrMut, Result, prelude::*};
use indexmap::{Equivalent, IndexMap};
use koto_api::{
    KotoCollection, KotoIdentity, KotoIndexSwap, KotoMap, KotoMapBuilder, KotoMapLookup,
    KotoMapSource, KotoMapSourceMut, KotoMetaMap, KotoSlice,
};
#[cfg(feature = "plugin")]
use koto_ffi as abi;
use rustc_hash::FxHasher;
use std::{
    hash::{BuildHasherDefault, Hash},
    ops::{Deref, DerefMut, RangeBounds},
};

/// The hasher used throughout the Koto runtime
pub type KotoHasher = FxHasher;

type ValueMapType = IndexMap<ValueKey, KValue, BuildHasherDefault<KotoHasher>>;

/// The (ValueKey -> Value) 'data' hash map used by the Koto runtime
///
/// See also: [KMap]
#[derive(Clone, Default)]
pub struct ValueMap(ValueMapType);

impl ValueMap {
    /// Creates a new map with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    /// Creates a new map containing a slice of the map's elements
    pub fn make_data_slice(&self, range: impl RangeBounds<usize>) -> Option<Self> {
        self.get_range(range).map(|entries| {
            Self::from_iter(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            )
        })
    }
}

impl Deref for ValueMap {
    type Target = ValueMapType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ValueMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<(ValueKey, KValue)> for ValueMap {
    fn from_iter<T: IntoIterator<Item = (ValueKey, KValue)>>(iter: T) -> ValueMap {
        Self(ValueMapType::from_iter(iter))
    }
}

/// The core hash map value type used in Koto, containing a [ValueMap] and a [MetaMap]
#[derive(Clone, Default)]
pub struct KMap {
    data: PtrMut<ValueMap>,
    meta: Option<PtrMut<MetaMap>>,
}

/// A borrowed view over a [`KMap`]'s data.
pub struct KMapData<'a>(Borrow<'a, ValueMap>);

/// A mutable borrowed view over a [`KMap`]'s data.
pub struct KMapDataMut<'a>(BorrowMut<'a, ValueMap>);

impl KMap {
    /// Creates an empty KMap
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty KMap, with a MetaMap containing the given @type string
    pub fn with_type(type_name: &str) -> Self {
        let mut meta = MetaMap::default();
        meta.insert(MetaKey::Type, type_name.into());
        Self::with_contents(ValueMap::default(), Some(meta))
    }

    /// Creates an empty KMap with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_contents(ValueMap::with_capacity(capacity), None)
    }

    /// Creates a KMap initialized with the provided data
    pub fn with_data(data: ValueMap) -> Self {
        Self::with_contents(data, None)
    }

    /// Creates a map from the provided key/value entries.
    ///
    /// Entries with non-hashable keys are ignored, matching the plugin host path.
    pub fn from_entries(entries: &[(KValue, KValue)]) -> Self {
        let data = entries
            .iter()
            .cloned()
            .filter_map(|(key, value)| ValueKey::try_from(key).ok().map(|key| (key, value)))
            .collect();
        Self::with_data(data)
    }

    /// Creates a KMap initialized with the provided data and meta map
    pub fn with_contents(data: ValueMap, meta: Option<MetaMap>) -> Self {
        Self {
            data: data.into(),
            meta: meta.map(PtrMut::from),
        }
    }

    /// Makes a KMap taking the data map from the first arg, and the meta map from the second
    pub fn from_data_and_meta_maps(data: &Self, meta: &Self) -> Self {
        Self {
            data: data.data.clone(),
            meta: meta.meta.clone(),
        }
    }

    /// Provides a reference to the data map
    pub fn data(&self) -> Borrow<'_, ValueMap> {
        self.data.borrow()
    }

    /// Provides a mutable reference to the data map
    pub fn data_mut(&self) -> BorrowMut<'_, ValueMap> {
        self.data.borrow_mut()
    }

    /// Provides a reference to the KMap's meta map
    ///
    /// This is returned as a reference to the meta map's PtrMut to allow for cloning.
    pub fn meta_map(&self) -> Option<&PtrMut<MetaMap>> {
        self.meta.as_ref()
    }

    /// Sets the KMap's meta map
    ///
    /// Note that this change isn't shared with maps that share the same data.
    pub fn set_meta_map(&mut self, meta: Option<PtrMut<MetaMap>>) {
        self.meta = meta;
    }

    /// Returns true if the meta map contains an entry with the given key
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        self.meta
            .as_ref()
            .is_some_and(|meta| meta.borrow().contains_key(key))
    }

    /// Returns a clone of the data value corresponding to the given key
    pub fn get<K>(&self, key: &K) -> Option<KValue>
    where
        K: Hash + Equivalent<ValueKey> + ?Sized,
    {
        self.data.borrow().get(key).cloned()
    }

    /// Returns a clone of the meta value corresponding to the given key
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<KValue> {
        self.meta
            .as_ref()
            .and_then(|meta| meta.borrow().get(key).cloned())
    }

    /// Insert an entry into the KMap's data
    pub fn insert(&self, key: impl Into<ValueKey>, value: impl Into<KValue>) {
        self.data_mut().insert(key.into(), value.into());
    }

    /// Remove an entry from KMap's data
    ///
    /// If a matching entry existed in the map then its value is returned.
    ///
    /// The order of entries in the map is preserved.
    pub fn remove(&self, key: impl Into<ValueKey>) -> Option<KValue> {
        self.data_mut().shift_remove(&key.into())
    }

    /// Removes a nested entry at the given `.` separated path
    ///
    /// If a matching entry existed in the map then its value is returned.
    ///
    /// The order of entries in the map is preserved.
    pub fn remove_path(&self, path: &str) -> Option<KValue> {
        if let Some((node, rest)) = path.split_once(".") {
            self.get(node)
                .and_then(|child| match child {
                    KValue::Map(map) => Some(map),
                    _ => None,
                })
                .and_then(|nested| nested.remove_path(rest))
        } else {
            self.remove(path)
        }
    }

    /// Inserts a value into the meta map, initializing the meta map if it doesn't yet exist
    pub fn insert_meta(&mut self, key: MetaKey, value: impl Into<KValue>) {
        self.meta
            .get_or_insert_with(Default::default)
            .borrow_mut()
            .insert(key, value.into());
    }

    /// Adds a function to the meta map.
    pub fn add_meta_fn(&mut self, key: MetaKey, f: impl KotoFunction) {
        self.insert_meta(key, KValue::NativeFunction(KNativeFunction::new(f)));
    }

    /// Adds a function to the KMap's data map
    pub fn add_fn(&self, id: &str, f: impl KotoFunction) {
        self.insert(id, KValue::NativeFunction(KNativeFunction::new(f)));
    }

    /// Returns the number of entries in the KMap's data map
    ///
    /// Note that this doesn't include entries in the meta map.
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Swaps two entries by index.
    pub fn swap_indices(&self, a: usize, b: usize) -> Result<()> {
        self.data_mut().swap_indices(a, b);
        Ok(())
    }

    /// Returns true if the KMap's data map contains no entries
    ///
    /// Note that this doesn't take entries in the meta map into account.
    pub fn is_empty(&self) -> bool {
        self.data().is_empty()
    }

    /// Removes all contents from the data map, and removes the meta map
    pub fn clear(&mut self) {
        self.data_mut().clear();
        self.meta = None;
    }

    /// Returns true if the provided KMap occupies the same memory address
    pub fn is_same_instance(&self, other: &Self) -> bool {
        PtrMut::ptr_eq(&self.data, &other.data)
    }

    /// If present, returns the @type meta value as a [KString], recursively going up the @base chain.
    pub fn meta_type(&self) -> Option<KString> {
        use KValue::*;

        match self.get_meta_value(&MetaKey::Type) {
            Some(Str(s)) => Some(s),
            Some(_) => Some("Error: expected string as result of @type".into()),
            None => match self.get_meta_value(&MetaKey::Base) {
                Some(Map(base)) => base.meta_type(),
                _ => None,
            },
        }
    }

    /// Renders the map to the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        if self.contains_meta_key(&UnaryOp::Display.into()) {
            let mut vm = ctx
                .vm()
                .ok_or_else(|| Error::from("missing VM in map display op"))?
                .spawn_shared_vm();
            match vm.run_unary_op(UnaryOp::Display, self.clone().into())? {
                KValue::Str(display_result) => {
                    ctx.append(display_result);
                }
                unexpected => return unexpected_type("String as @display result", &unexpected),
            }
        } else {
            if let Some(meta_type) = self.meta_type() {
                ctx.append(meta_type);
                ctx.append(' ');
            }

            ctx.append('{');

            let id = PtrMut::address(&self.data);

            if ctx.is_in_parents(id) {
                ctx.append("...");
            } else {
                ctx.push_container(id);

                for (i, (key, value)) in self.data().iter().enumerate() {
                    if i > 0 {
                        ctx.append(", ");
                    }

                    let mut key_ctx = DisplayContext::default();
                    key.value().display(&mut key_ctx)?;
                    ctx.append(key_ctx.result());
                    ctx.append(": ");

                    value.display(ctx)?;
                }

                ctx.pop_container();
            }

            ctx.append('}');
        }

        Ok(())
    }
}

#[cfg(feature = "plugin")]
impl AbiTransfer for KMap {
    type Abi = abi::KMap;

    unsafe fn into_abi(self) -> Self::Abi {
        abi::KMap {
            data: unsafe { PtrMut::into_raw(self.data) } as *mut _,
            meta: self
                .meta
                .map(|meta| unsafe { PtrMut::into_raw(meta) } as *mut _)
                .unwrap_or(std::ptr::null_mut()),
        }
    }

    unsafe fn from_abi(map: Self::Abi) -> Self {
        Self {
            data: unsafe { PtrMut::from_raw(map.data as *const KCell<ValueMap>) },
            meta: (!map.meta.is_null())
                .then(|| unsafe { PtrMut::from_raw(map.meta as *const KCell<MetaMap>) }),
        }
    }

    unsafe fn clone_from_abi(map: Self::Abi) -> Self {
        Self {
            data: unsafe { PtrMut::clone_from_raw(map.data as *const KCell<ValueMap>) },
            meta: (!map.meta.is_null())
                .then(|| unsafe { PtrMut::clone_from_raw(map.meta as *const KCell<MetaMap>) }),
        }
    }
}

impl From<ValueMap> for KMap {
    fn from(value: ValueMap) -> Self {
        KMap::with_data(value)
    }
}

impl KMapDataMut<'_> {
    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl KotoCollection<RuntimeBackend> for KMapData<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl KotoSlice<RuntimeBackend> for KMapData<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index).map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoMapLookup<RuntimeBackend> for KMapData<'_> {
    fn get_key(&self, key: &KValue) -> Option<KValue> {
        let key = ValueKey::try_from(key.clone()).ok()?;
        self.0.get(&key).cloned()
    }
}

impl KotoMap<RuntimeBackend> for KMapData<'_> {
    fn get_index(&self, index: usize) -> Option<(KValue, KValue)> {
        self.0
            .get_index(index)
            .map(|(key, value)| (key.value().clone(), value.clone()))
    }
}

impl KotoCollection<RuntimeBackend> for KMapDataMut<'_> {
    fn len(&self) -> usize {
        KMapDataMut::len(self)
    }
}

impl KotoSlice<RuntimeBackend> for KMapDataMut<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index).map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoMapLookup<RuntimeBackend> for KMapDataMut<'_> {
    fn get_key(&self, key: &KValue) -> Option<KValue> {
        let key = ValueKey::try_from(key.clone()).ok()?;
        self.0.get(&key).cloned()
    }
}

impl KotoMap<RuntimeBackend> for KMapDataMut<'_> {
    fn get_index(&self, index: usize) -> Option<(KValue, KValue)> {
        self.0
            .get_index(index)
            .map(|(key, value)| (key.value().clone(), value.clone()))
    }
}

impl KotoIndexSwap<RuntimeBackend> for KMapDataMut<'_> {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        self.0.swap_indices(a, b);
        Ok(())
    }
}

impl KotoCollection<RuntimeBackend> for KMap {
    fn len(&self) -> usize {
        self.data().len()
    }
}

impl KotoMapSource<RuntimeBackend> for KMap {
    type Data<'a>
        = KMapData<'a>
    where
        Self: 'a;

    fn data(&self) -> Self::Data<'_> {
        KMapData(KMap::data(self))
    }

    fn from_entries(entries: &[(KValue, KValue)]) -> Self {
        KMap::from_entries(entries)
    }
}

impl KotoSlice<RuntimeBackend> for KMap {
    fn get(&self, index: usize) -> Option<KValue> {
        self.get_index(index)
            .map(|entry| KTuple::from(vec![entry.0, entry.1]).into())
    }
}

impl KotoIdentity for KMap {
    fn is_same_instance(&self, other: &Self) -> bool {
        KMap::is_same_instance(self, other)
    }
}

impl KotoIndexSwap<RuntimeBackend> for KMap {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        KMap::swap_indices(self, a, b)
    }
}

impl KotoMapSourceMut<RuntimeBackend> for KMap {
    type DataMut<'a>
        = KMapDataMut<'a>
    where
        Self: 'a;

    fn data_mut(&self) -> Self::DataMut<'_> {
        KMapDataMut(KMap::data_mut(self))
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

impl From<Vec<(KValue, KValue)>> for KMap {
    fn from(entries: Vec<(KValue, KValue)>) -> Self {
        KMap::from_entries(&entries)
    }
}

impl FromIterator<(KValue, KValue)> for KMap {
    fn from_iter<T: IntoIterator<Item = (KValue, KValue)>>(iter: T) -> Self {
        KMap::from(iter.into_iter().collect::<Vec<_>>())
    }
}

impl KotoMapBuilder<RuntimeBackend> for KMap {
    type MetaKey = MetaKey;

    fn with_type(type_name: &str) -> Self {
        KMap::with_type(type_name)
    }

    fn insert(&self, key: &str, value: impl Into<KValue>) {
        self.insert(key, value);
    }

    fn add_fn<F>(&self, key: &str, f: F)
    where
        F: for<'a> Fn(&mut crate::CallContext<'a>) -> std::result::Result<KValue, crate::Error>
            + Send
            + Sync
            + 'static,
    {
        self.add_fn(key, f);
    }

    fn insert_meta(&mut self, key: Self::MetaKey, value: impl Into<KValue>) {
        self.insert_meta(key, value.into());
    }

    fn add_meta_fn<F>(&mut self, key: Self::MetaKey, f: F)
    where
        F: for<'a> Fn(&mut crate::CallContext<'a>) -> std::result::Result<KValue, crate::Error>
            + Send
            + Sync
            + 'static,
    {
        self.add_meta_fn(key, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_remove_with_string() {
        let m = KMap::default();

        assert!(m.get("test").is_none());
        m.insert("test", KValue::Null);
        assert!(m.get("test").is_some());
        assert!(matches!(m.remove("test"), Some(KValue::Null)));
        assert!(m.get("test").is_none());
    }

    #[test]
    fn remove_path() {
        let b = KMap::default();
        b.insert("c", KValue::Null);
        b.insert("d", KValue::Null);

        let a = KMap::default();
        a.insert("b", b.clone());

        let x = KMap::default();
        x.insert("a", a);

        x.remove_path("a.b.c");

        // `b` should now have had it's `c` entry removed
        assert!(b.get("c").is_none());
        assert!(b.get("d").is_some());
    }
}
