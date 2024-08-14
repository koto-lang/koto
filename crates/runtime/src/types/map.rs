use crate::{prelude::*, Borrow, BorrowMut, Error, PtrMut, Result};
use indexmap::{Equivalent, IndexMap};
use rustc_hash::FxHasher;
use std::{
    hash::{BuildHasherDefault, Hash},
    ops::{Deref, DerefMut, RangeBounds},
};

/// The hasher used throughout the Koto runtime
pub type KotoHasher = FxHasher;

type ValueMapType = IndexMap<ValueKey, KValue, BuildHasherDefault<KotoHasher>>;

/// The (ValueKey -> Value) 'data' hashmap used by the Koto runtime
///
/// See also: [KMap]
#[derive(Clone, Default)]
pub struct ValueMap(ValueMapType);

impl ValueMap {
    /// Creates a new DataMap with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    /// Makes a new ValueMap containing a slice of the map's elements
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

/// The core hashmap value type used in Koto, containing a [ValueMap] and a [MetaMap]
#[derive(Clone, Default)]
pub struct KMap {
    data: PtrMut<ValueMap>,
    meta: Option<PtrMut<MetaMap>>,
}

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
    pub fn data(&self) -> Borrow<ValueMap> {
        self.data.borrow()
    }

    /// Provides a mutable reference to the data map
    pub fn data_mut(&self) -> BorrowMut<ValueMap> {
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
            .map_or(false, |meta| meta.borrow().contains_key(key))
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

    /// Inserts a value into the meta map, initializing the meta map if it doesn't yet exist
    pub fn insert_meta(&mut self, key: MetaKey, value: KValue) {
        self.meta
            .get_or_insert_with(Default::default)
            .borrow_mut()
            .insert(key, value);
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

    /// Renders the map to the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        if self.contains_meta_key(&UnaryOp::Display.into()) {
            let mut vm = ctx
                .vm()
                .ok_or_else(|| Error::from("Missing VM in map display op"))?
                .spawn_shared_vm();
            match vm.run_unary_op(UnaryOp::Display, self.clone().into())? {
                KValue::Str(display_result) => {
                    ctx.append(display_result);
                }
                unexpected => return unexpected_type("String as @display result", &unexpected),
            }
        } else {
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

impl From<ValueMap> for KMap {
    fn from(value: ValueMap) -> Self {
        KMap::with_data(value)
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
        assert!(matches!(
            m.data_mut().shift_remove("test"),
            Some(KValue::Null)
        ));
        assert!(m.get("test").is_none());
    }
}
