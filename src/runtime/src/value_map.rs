use {
    crate::{
        external::{ArgRegisters, ExternalFunction},
        value_key::ValueKeyRef,
        MetaKey, MetaMap, RuntimeResult, Value, ValueKey, ValueList, Vm,
    },
    indexmap::IndexMap,
    rustc_hash::FxHasher,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        hash::BuildHasherDefault,
        iter::IntoIterator,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
};

type DataMapType = IndexMap<ValueKey, Value, BuildHasherDefault<FxHasher>>;

/// The underlying ValueKey -> Value 'data' hash map used in Koto
///
/// See also: [ValueMap]
#[derive(Clone, Debug, Default)]
pub struct DataMap(DataMapType);

impl DataMap {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(DataMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        #[allow(clippy::useless_conversion)]
        self.add_value(
            id.into(),
            Value::ExternalFunction(ExternalFunction::new(f, false)),
        );
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    pub fn add_list(&mut self, id: &str, list: ValueList) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id.into(), Value::List(list));
    }

    pub fn add_map(&mut self, id: &str, map: ValueMap) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id.into(), Value::Map(map));
    }

    pub fn add_value(&mut self, id: &str, value: Value) -> Option<Value> {
        #[allow(clippy::useless_conversion)]
        self.insert(id.into(), value)
    }

    /// Allows access to map entries without having to create a ValueString
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn ValueKeyRef)
    }

    /// Allows access to map entries without having to create a ValueString
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn ValueKeyRef)
    }

    /// Removes any entry with a matching name and returns the removed value
    pub fn remove_with_string(&mut self, key: &str) -> Option<Value> {
        self.0.remove(&key as &dyn ValueKeyRef)
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
    data: Rc<RefCell<DataMap>>,
    meta: Option<Rc<RefCell<MetaMap>>>,
}

impl ValueMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_contents(DataMap::with_capacity(capacity), None)
    }

    pub fn with_data(data: DataMap) -> Self {
        Self::with_contents(data, None)
    }

    pub fn with_contents(data: DataMap, meta: Option<MetaMap>) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: meta.map(|meta| Rc::new(RefCell::new(meta))),
        }
    }

    // Makes a ValueMap taking the data map from the first arg, and the meta map from the second
    pub fn from_data_and_meta_maps(data: &Self, meta: &Self) -> Self {
        Self {
            data: data.data.clone(),
            meta: meta.meta.clone(),
        }
    }

    pub fn data(&self) -> Ref<DataMap> {
        self.data.borrow()
    }

    pub fn data_mut(&self) -> RefMut<DataMap> {
        self.data.borrow_mut()
    }

    pub fn meta_map(&self) -> Option<&Rc<RefCell<MetaMap>>> {
        self.meta.as_ref()
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

    pub fn len(&self) -> usize {
        self.data().len()
    }

    pub fn is_empty(&self) -> bool {
        self.data().is_empty()
    }

    pub fn add_fn(&self, id: &str, f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &self,
        id: &str,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    pub fn add_list(&self, id: &str, list: ValueList) {
        self.add_value(id, Value::List(list));
    }

    pub fn add_map(&self, id: &str, map: ValueMap) {
        self.add_value(id, Value::Map(map));
    }

    pub fn add_value(&self, id: &str, value: Value) {
        self.insert(id.into(), value);
    }
}

impl fmt::Display for ValueMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for (key, value) in self.data().iter() {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}: {value:#}", key.value())?;
            first = false;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_remove_with_string() {
        let m = ValueMap::default();
        let mut data = m.data_mut();

        assert!(data.get_with_string("test").is_none());
        data.add_value("test", Value::Null);
        assert!(data.get_with_string("test").is_some());
        assert!(matches!(data.remove_with_string("test"), Some(Value::Null)));
        assert!(data.get_with_string("test").is_none());
    }
}
