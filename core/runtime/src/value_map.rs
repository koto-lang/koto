use {
    crate::{
        external_function::{ArgRegisters, ExternalFunction},
        prelude::*,
    },
    indexmap::IndexMap,
    rustc_hash::FxHasher,
    std::{
        hash::BuildHasherDefault,
        iter::IntoIterator,
        ops::{Deref, DerefMut},
    },
};

/// The hasher used throughout the Koto runtime
pub type KotoHasher = FxHasher;

type DataMapType = IndexMap<ValueKey, Value, BuildHasherDefault<KotoHasher>>;

/// The underlying ValueKey -> Value 'data' hash map used in Koto
///
/// See also: [ValueMap]
#[derive(Clone, Debug, Default)]
pub struct DataMap(DataMapType);

impl DataMap {
    /// Creates a new DataMap with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(DataMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }
}

impl Deref for DataMap {
    type Target = DataMapType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DataMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<(ValueKey, Value)> for DataMap {
    fn from_iter<T: IntoIterator<Item = (ValueKey, Value)>>(iter: T) -> DataMap {
        Self(DataMapType::from_iter(iter))
    }
}

/// The Map value type used in Koto
#[derive(Clone, Debug, Default)]
pub struct ValueMap {
    data: PtrMut<DataMap>,
    meta: Option<PtrMut<MetaMap>>,
}

impl ValueMap {
    /// Creates an empty ValueMap
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty ValueMap with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_contents(DataMap::with_capacity(capacity), None)
    }

    /// Creates a ValueMap initialized with the provided data
    pub fn with_data(data: DataMap) -> Self {
        Self::with_contents(data, None)
    }

    /// Creates a ValueMap initialized with the provided data and meta map
    pub fn with_contents(data: DataMap, meta: Option<MetaMap>) -> Self {
        Self {
            data: data.into(),
            meta: meta.map(PtrMut::from),
        }
    }

    /// Makes a ValueMap taking the data map from the first arg, and the meta map from the second
    pub fn from_data_and_meta_maps(data: &Self, meta: &Self) -> Self {
        Self {
            data: data.data.clone(),
            meta: meta.meta.clone(),
        }
    }

    /// Provides a reference to the ValueMaps' data
    pub fn data(&self) -> Borrow<DataMap> {
        self.data.borrow()
    }

    /// Provides a mutable reference to the ValueMaps' data
    pub fn data_mut(&self) -> BorrowMut<DataMap> {
        self.data.borrow_mut()
    }

    /// Provides a reference to the ValueMap's meta map
    ///
    /// This is returned as a reference to the meta map's Rc to allow for cloning.
    pub fn meta_map(&self) -> Option<&PtrMut<MetaMap>> {
        self.meta.as_ref()
    }

    /// Sets the ValueMap's meta map
    pub fn set_meta_map(&mut self, meta: Option<MetaMap>) {
        self.meta = meta.map(PtrMut::from)
    }

    /// Returns true if the meta map contains an entry with the given key
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        self.meta
            .as_ref()
            .map_or(false, |meta| meta.borrow().contains_key(key))
    }

    /// Returns a clone of the meta value corresponding to the given key
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<Value> {
        self.meta
            .as_ref()
            .and_then(|meta| meta.borrow().get(key).cloned())
    }

    /// Insert an entry into the ValueMap's data
    pub fn insert(&self, key: ValueKey, value: Value) {
        self.data_mut().insert(key, value);
    }

    /// Inserts a value into the meta map, initializing the meta map if it doesn't yet exist
    pub fn insert_meta(&mut self, key: MetaKey, value: Value) {
        self.meta
            .get_or_insert_with(Default::default)
            .borrow_mut()
            .insert(key, value);
    }

    /// Adds a function to the ValueMap's data map
    pub fn add_fn(&self, id: &str, f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    /// Adds an instance function to the ValueMap's data map
    pub fn add_instance_fn(
        &self,
        id: &str,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    /// Adds a map to the ValueMap's data map
    pub fn add_map(&self, id: &str, map: ValueMap) {
        self.add_value(id, Value::Map(map));
    }

    /// Adds a [Value](crate::Value) to the ValueMap's data map
    pub fn add_value(&self, id: &str, value: Value) {
        self.insert(id.into(), value);
    }

    /// Returns the number of entries in the ValueMap's data map
    ///
    /// Note that this doesn't include entries in the meta map.
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Returns true if the ValueMap's data map contains no entries
    ///
    /// Note that this doesn't take entries in the meta map into account.
    pub fn is_empty(&self) -> bool {
        self.data().is_empty()
    }
}

impl KotoDisplay for ValueMap {
    fn display(&self, s: &mut String, vm: &mut Vm, _options: KotoDisplayOptions) -> RuntimeResult {
        if self.contains_meta_key(&UnaryOp::Display.into()) {
            match vm.run_unary_op(UnaryOp::Display, self.clone().into())? {
                Value::Str(display_result) => s.push_str(&display_result),
                unexpected => return type_error("String as @display result", &unexpected),
            }
        } else {
            s.push('{');
            for (i, (key, value)) in self.data().iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                key.value().display(s, vm, KotoDisplayOptions::default())?;
                s.push_str(": ");
                value.display(
                    s,
                    vm,
                    KotoDisplayOptions {
                        contained_value: true,
                    },
                )?;
            }
            s.push('}');
        }

        Ok(().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_remove_with_string() {
        let m = ValueMap::default();

        assert!(m.data().get("test").is_none());
        m.add_value("test", Value::Null);
        assert!(m.data().get("test").is_some());
        assert!(matches!(m.data_mut().remove("test"), Some(Value::Null)));
        assert!(m.data().get("test").is_none());
    }
}
