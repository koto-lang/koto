use {
    crate::{
        external::{Args, ExternalFunction},
        value_key::ValueKeyRef,
        MetaMap, RuntimeResult, Value, ValueKey, ValueList, Vm,
    },
    indexmap::IndexMap,
    parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    rustc_hash::FxHasher,
    std::{
        fmt,
        hash::BuildHasherDefault,
        iter::{FromIterator, IntoIterator},
        ops::{Deref, DerefMut},
        sync::Arc,
    },
};

type ValueHashMapType = IndexMap<ValueKey, Value, BuildHasherDefault<FxHasher>>;

/// The underlying ValueKey -> Value 'data' hash map used in Koto
///
/// See also: [ValueMap]
#[repr(C)]
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
    pub fn extend(&mut self, other: &ValueHashMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    #[inline]
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn ValueKeyRef)
    }

    #[inline]
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn ValueKeyRef)
    }
}

impl Deref for ValueHashMap {
    type Target = ValueHashMapType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ValueHashMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<(ValueKey, Value)> for ValueHashMap {
    #[inline]
    fn from_iter<T: IntoIterator<Item = (ValueKey, Value)>>(iter: T) -> ValueHashMap {
        Self(ValueHashMapType::from_iter(iter))
    }
}

/// The contents of a ValueMap, combining a data map with a meta map
#[derive(Clone, Debug, Default)]
pub struct ValueMapContents {
    pub data: ValueHashMap,
    pub meta: MetaMap,
}

impl ValueMapContents {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: ValueHashMap::with_capacity(capacity),
            meta: MetaMap::default(),
        }
    }

    #[inline]
    pub fn with_data(data: ValueHashMap) -> Self {
        Self {
            data,
            meta: MetaMap::default(),
        }
    }

    pub fn extend(&mut self, other: &ValueMapContents) {
        self.data.extend(&other.data);
        self.meta.extend(other.meta.clone().into_iter());
    }
}

/// The Map value type used in Koto
#[derive(Clone, Debug, Default)]
pub struct ValueMap(Arc<RwLock<ValueMapContents>>);

impl ValueMap {
    #[inline]
    pub fn new() -> Self {
        Self::with_contents(ValueMapContents::default())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_contents(ValueMapContents::with_capacity(capacity))
    }

    #[inline]
    pub fn with_data(data: ValueHashMap) -> Self {
        Self::with_contents(ValueMapContents::with_data(data))
    }

    #[inline]
    pub fn with_contents(contents: ValueMapContents) -> Self {
        Self(Arc::new(RwLock::new(contents)))
    }

    #[inline]
    pub fn contents(&self) -> RwLockReadGuard<ValueMapContents> {
        self.0.read()
    }

    #[inline]
    pub fn contents_mut(&self) -> RwLockWriteGuard<ValueMapContents> {
        self.0.write()
    }

    #[inline]
    pub fn insert(&mut self, key: ValueKey, value: Value) {
        self.contents_mut().data.insert(key, value);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.contents().data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.contents().data.is_empty()
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
        if f.alternate() {
            for (key, value) in self.contents().data.iter() {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}: {:#}", key.value(), value)?;
                first = false;
            }
        } else {
            for key in self.contents().data.keys() {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", key.value())?;
                first = false;
            }
        }
        write!(f, "}}")
    }
}

pub struct ValueMapIter<'map> {
    map: &'map RwLock<ValueMapContents>,
    index: usize,
}

impl<'map> ValueMapIter<'map> {
    #[inline]
    fn new(map: &'map RwLock<ValueMapContents>) -> Self {
        Self { map, index: 0 }
    }
}

impl<'map> Iterator for ValueMapIter<'map> {
    type Item = (ValueKey, Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.map.read().data.get_index(self.index) {
            Some((key, value)) => {
                self.index += 1;
                Some((key.clone(), value.clone()))
            }
            None => None,
        }
    }
}
