use {
    crate::{external::ExternalFunction, RuntimeResult, Value, ValueList, Vm},
    indexmap::{
        map::{Iter, Keys, Values},
        IndexMap,
    },
    rustc_hash::FxHasher,
    std::{
        fmt,
        hash::BuildHasherDefault,
        iter::{FromIterator, IntoIterator},
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
};

type ValueHashMapType = IndexMap<Value, Value, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug, Default)]
pub struct ValueHashMap(ValueHashMapType);

impl ValueHashMap {
    pub fn new() -> Self {
        Self(ValueHashMapType::default())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueHashMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(
            id.into(),
            Value::ExternalFunction(ExternalFunction::new(f, false)),
        );
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(
            id.into(),
            Value::ExternalFunction(ExternalFunction::new(f, true)),
        );
    }

    pub fn add_list(&mut self, id: &str, list: ValueList) {
        self.add_value(id.into(), Value::List(list));
    }

    pub fn add_map(&mut self, id: &str, map: ValueMap) {
        self.add_value(id.into(), Value::Map(map));
    }

    pub fn add_value(&mut self, id: &str, value: Value) -> Option<Value> {
        self.insert(id.into(), value)
    }

    pub fn insert(&mut self, key: Value, value: Value) -> Option<Value> {
        self.0.insert(key, value)
    }

    pub fn remove(&mut self, key: &Value) -> Option<Value> {
        self.0.remove(key)
    }

    pub fn extend(&mut self, other: &ValueHashMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn get_with_string(&self, key: &Arc<String>) -> Option<&Value> {
        self.0.get(&Value::Str(key.clone()))
    }

    pub fn get_mut(&mut self, key: &Value) -> Option<&mut Value> {
        self.0.get_mut(key)
    }

    pub fn get_index(&self, index: usize) -> Option<(&Value, &Value)> {
        self.0.get_index(index)
    }

    pub fn contains_key(&self, key: &Value) -> bool {
        self.0.contains_key(key)
    }

    pub fn keys(&self) -> Keys<'_, Value, Value> {
        self.0.keys()
    }

    pub fn values(&self) -> Values<'_, Value, Value> {
        self.0.values()
    }

    pub fn iter(&self) -> Iter<'_, Value, Value> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl FromIterator<(Value, Value)> for ValueHashMap {
    fn from_iter<T: IntoIterator<Item = (Value, Value)>>(iter: T) -> ValueHashMap {
        Self(ValueHashMapType::from_iter(iter))
    }
}

impl PartialEq for ValueHashMap {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ValueHashMap {}

#[derive(Clone, Debug, Default)]
pub struct ValueMap(Arc<RwLock<ValueHashMap>>);

impl ValueMap {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ValueHashMap::default())))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(RwLock::new(ValueHashMap::with_capacity(capacity))))
    }

    pub fn with_data(data: ValueHashMap) -> Self {
        Self(Arc::new(RwLock::new(data)))
    }

    pub fn data(&self) -> RwLockReadGuard<ValueHashMap> {
        self.0.read().unwrap()
    }

    pub fn data_mut(&self) -> RwLockWriteGuard<ValueHashMap> {
        self.0.write().unwrap()
    }

    pub fn insert(&mut self, key: Value, value: Value) {
        self.data_mut().insert(key, value);
    }

    pub fn len(&self) -> usize {
        self.data().len()
    }

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    pub fn add_list(&mut self, id: &str, list: ValueList) {
        self.add_value(id, Value::List(list));
    }

    pub fn add_map(&mut self, id: &str, map: ValueMap) {
        self.add_value(id, Value::Map(map));
    }

    pub fn add_value(&mut self, id: &str, value: Value) {
        self.insert(id.into(), value);
    }

    // An iterator that clones the map's keys and values
    //
    // Useful for avoiding holding on to the underlying RwLock while iterating
    pub fn cloned_iter(&self) -> ValueMapIter {
        ValueMapIter::new(&self.0)
    }
}

impl fmt::Display for ValueMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for (key, _value) in self.data().iter() {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", key)?;
            first = false;
        }
        write!(f, "}}")
    }
}

impl PartialEq for ValueMap {
    fn eq(&self, other: &Self) -> bool {
        *self.data() == *other.data()
    }
}
impl Eq for ValueMap {}

pub struct ValueMapIter<'map> {
    map: &'map RwLock<ValueHashMap>,
    index: usize,
}

impl<'map> ValueMapIter<'map> {
    fn new(map: &'map RwLock<ValueHashMap>) -> Self {
        Self { map, index: 0 }
    }
}

impl<'map> Iterator for ValueMapIter<'map> {
    type Item = (Value, Value);

    fn next(&mut self) -> Option<Self::Item> {
        match self.map.read().unwrap().get_index(self.index) {
            Some((key, value)) => {
                self.index += 1;
                Some((key.clone(), value.clone()))
            }
            None => None,
        }
    }
}
