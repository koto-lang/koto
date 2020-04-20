use crate::{
    external::{ExternalFunction, ExternalValue},
    value::make_external_value,
    Id, Runtime, RuntimeResult, Value, ValueList, EXTERNAL_DATA_ID,
};
use rustc_hash::FxHashMap;
use std::{
    borrow::Borrow,
    collections::hash_map::{Iter, Keys},
    hash::Hash,
    iter::{FromIterator, IntoIterator},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[derive(Clone, Debug, Default)]
pub struct ValueHashMap(FxHashMap<Id, Value>);

impl ValueHashMap {
    pub fn new() -> Self {
        Self(FxHashMap::default())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(FxHashMap::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Runtime, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Runtime, &[Value]) -> RuntimeResult + Send + Sync + 'static,
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
        self.insert(Id::from_str(id), value);
    }

    pub fn insert(&mut self, id: Id, value: Value) {
        self.0.insert(id, value);
    }

    pub fn extend(&mut self, other: &ValueHashMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    pub fn get<K: ?Sized>(&self, id: &K) -> Option<&Value>
    where
        Id: Borrow<K>,
        K: Hash + Eq,
    {
        self.0.get(id)
    }

    pub fn get_mut<K: ?Sized>(&mut self, id: &K) -> Option<&mut Value>
    where
        Id: Borrow<K>,
        K: Hash + Eq,
    {
        self.0.get_mut(id)
    }

    pub fn contains_key(&self, id: &str) -> bool {
        self.0.contains_key(id)
    }

    pub fn keys(&self) -> Keys<'_, Id, Value> {
        self.0.keys()
    }

    pub fn iter(&self) -> Iter<'_, Id, Value> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<(Id, Value)> for ValueHashMap {
    fn from_iter<T: IntoIterator<Item = (Id, Value)>>(iter: T) -> ValueHashMap {
        Self(FxHashMap::from_iter(iter))
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

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Runtime, &[Value]) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Runtime, &[Value]) -> RuntimeResult + Send + Sync + 'static,
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
        self.insert(Id::from_str(id), value);
    }

    pub fn set_external_value(&mut self, data: impl ExternalValue) {
        self.add_value(EXTERNAL_DATA_ID, make_external_value(data));
    }

    pub fn insert(&mut self, name: Id, value: Value) {
        self.data_mut().insert(name, value);
    }
}

impl PartialEq for ValueMap {
    fn eq(&self, other: &Self) -> bool {
        *self.data() == *other.data()
    }
}
impl Eq for ValueMap {}
