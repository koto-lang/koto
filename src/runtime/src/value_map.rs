use {
    crate::{
        external::{Args, ExternalFunction},
        RuntimeResult, Value, ValueList, ValueRef, Vm,
    },
    indexmap::{
        map::{Iter, Keys, Values},
        IndexMap,
    },
    parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    rustc_hash::FxHasher,
    std::{
        borrow::Borrow,
        fmt,
        hash::{BuildHasherDefault, Hash, Hasher},
        iter::{FromIterator, IntoIterator},
        sync::Arc,
        // sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
};

pub trait ValueMapKey {
    fn to_value_ref(&self) -> ValueRef;
}

impl<'a> Hash for dyn ValueMapKey + 'a {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_value_ref().hash(state);
    }
}

impl<'a> PartialEq for dyn ValueMapKey + 'a {
    fn eq(&self, other: &Self) -> bool {
        self.to_value_ref() == other.to_value_ref()
    }
}

impl<'a> Eq for dyn ValueMapKey + 'a {}

impl ValueMapKey for Value {
    fn to_value_ref(&self) -> ValueRef {
        self.as_ref()
    }
}

impl<'a> ValueMapKey for &'a str {
    fn to_value_ref(&self) -> ValueRef {
        ValueRef::Str(self)
    }
}

impl<'a> Borrow<dyn ValueMapKey + 'a> for Value {
    fn borrow(&self) -> &(dyn ValueMapKey + 'a) {
        self
    }
}

impl<'a> Borrow<dyn ValueMapKey + 'a> for &'a str {
    fn borrow(&self) -> &(dyn ValueMapKey + 'a) {
        self
    }
}

type ValueHashMapType = IndexMap<Value, Value, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug, Default)]
pub struct ValueHashMap(ValueHashMapType);

impl ValueHashMap {
    #[inline]
    pub fn new() -> Self {
        Self(ValueHashMapType::default())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueHashMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    #[inline]
    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static,
    ) {
        #[allow(clippy::useless_conversion)]
        self.add_value(
            id.into(),
            Value::ExternalFunction(ExternalFunction::new(f, false)),
        );
    }

    #[inline]
    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static,
    ) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    #[inline]
    pub fn add_list(&mut self, id: &str, list: ValueList) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id.into(), Value::List(list));
    }

    #[inline]
    pub fn add_map(&mut self, id: &str, map: ValueMap) {
        #[allow(clippy::useless_conversion)]
        self.add_value(id.into(), Value::Map(map));
    }

    #[inline]
    pub fn add_value(&mut self, id: &str, value: Value) -> Option<Value> {
        #[allow(clippy::useless_conversion)]
        self.insert(id.into(), value)
    }

    #[inline]
    pub fn insert(&mut self, key: Value, value: Value) -> Option<Value> {
        #[allow(clippy::useless_conversion)]
        self.0.insert(key.into(), value)
    }

    #[inline]
    pub fn remove(&mut self, key: &Value) -> Option<Value> {
        self.0.remove(key)
    }

    #[inline]
    pub fn extend(&mut self, other: &ValueHashMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.0.get(key)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &dyn ValueMapKey) -> Option<&mut Value> {
        self.0.get_mut(key)
    }

    #[inline]
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn ValueMapKey)
    }

    #[inline]
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn ValueMapKey)
    }

    #[inline]
    pub fn get_index(&self, index: usize) -> Option<(&Value, &Value)> {
        self.0.get_index(index)
    }

    #[inline]
    pub fn contains_key(&self, key: &dyn ValueMapKey) -> bool {
        self.0.contains_key(key)
    }

    #[inline]
    pub fn keys(&self) -> Keys<'_, Value, Value> {
        self.0.keys()
    }

    #[inline]
    pub fn values(&self) -> Values<'_, Value, Value> {
        self.0.values()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, Value, Value> {
        self.0.iter()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl FromIterator<(Value, Value)> for ValueHashMap {
    #[inline]
    fn from_iter<T: IntoIterator<Item = (Value, Value)>>(iter: T) -> ValueHashMap {
        Self(ValueHashMapType::from_iter(iter))
    }
}

impl PartialEq for ValueHashMap {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ValueHashMap {}

#[derive(Clone, Debug, Default)]
pub struct ValueMap(Arc<RwLock<ValueHashMap>>);

impl ValueMap {
    #[inline]
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ValueHashMap::default())))
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(RwLock::new(ValueHashMap::with_capacity(capacity))))
    }

    #[inline]
    pub fn with_data(data: ValueHashMap) -> Self {
        Self(Arc::new(RwLock::new(data)))
    }

    #[inline]
    pub fn data(&self) -> RwLockReadGuard<ValueHashMap> {
        self.0.read()
    }

    #[inline]
    pub fn data_mut(&self) -> RwLockWriteGuard<ValueHashMap> {
        self.0.write()
    }

    #[inline]
    pub fn insert(&mut self, key: Value, value: Value) {
        self.data_mut().insert(key, value);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data().is_empty()
    }

    #[inline]
    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    #[inline]
    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static,
    ) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    #[inline]
    pub fn add_list(&mut self, id: &str, list: ValueList) {
        self.add_value(id, Value::List(list));
    }

    #[inline]
    pub fn add_map(&mut self, id: &str, map: ValueMap) {
        self.add_value(id, Value::Map(map));
    }

    #[inline]
    pub fn add_value(&mut self, id: &str, value: Value) {
        self.insert(id.into(), value);
    }

    // An iterator that clones the map's keys and values
    //
    // Useful for avoiding holding on to the underlying RwLock while iterating
    #[inline]
    pub fn cloned_iter(&self) -> ValueMapIter {
        ValueMapIter::new(&self.0)
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
            write!(f, "{}: {:#}", key, value)?;
            first = false;
        }
        write!(f, "}}")
    }
}

impl PartialEq for ValueMap {
    #[inline]
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
    #[inline]
    fn new(map: &'map RwLock<ValueHashMap>) -> Self {
        Self { map, index: 0 }
    }
}

impl<'map> Iterator for ValueMapIter<'map> {
    type Item = (Value, Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.map.read().get_index(self.index) {
            Some((key, value)) => {
                self.index += 1;
                Some((key.clone(), value.clone()))
            }
            None => None,
        }
    }
}
