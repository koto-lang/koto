use {
    crate::{
        external::{Args, ExternalFunction},
        value_key::ValueKeyRef,
        MetaMap, RuntimeResult, Value, ValueKey, ValueList, Vm,
    },
    indexmap::IndexMap,
    rustc_hash::FxHasher,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        hash::BuildHasherDefault,
        iter::{FromIterator, IntoIterator},
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
    #[inline]
    pub fn new() -> Self {
        Self(DataMapType::default())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(DataMapType::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    #[inline]
    pub fn add_fn(&mut self, id: &str, f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static) {
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
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static,
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
    pub fn extend(&mut self, other: &DataMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    /// Allows access to map entries without having to create a ValueString
    #[inline]
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn ValueKeyRef)
    }

    /// Allows access to map entries without having to create a ValueString
    #[inline]
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn ValueKeyRef)
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
    #[inline]
    fn from_iter<T: IntoIterator<Item = (ValueKey, Value)>>(iter: T) -> DataMap {
        Self(DataMapType::from_iter(iter))
    }
}

/// The Map value type used in Koto
#[derive(Clone, Debug, Default)]
pub struct ValueMap {
    data: Rc<RefCell<DataMap>>,
    meta: Rc<RefCell<MetaMap>>,
}

impl ValueMap {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_contents(DataMap::with_capacity(capacity), MetaMap::default())
    }

    #[inline]
    pub fn with_data(data: DataMap) -> Self {
        Self::with_contents(data, MetaMap::default())
    }

    #[inline]
    pub fn with_contents(data: DataMap, meta: MetaMap) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: Rc::new(RefCell::new(meta)),
        }
    }

    #[inline]
    pub fn data(&self) -> Ref<DataMap> {
        self.data.borrow()
    }

    #[inline]
    pub fn data_mut(&self) -> RefMut<DataMap> {
        self.data.borrow_mut()
    }

    #[inline]
    pub fn meta(&self) -> Ref<MetaMap> {
        self.meta.borrow()
    }

    #[inline]
    pub fn meta_mut(&self) -> RefMut<MetaMap> {
        self.meta.borrow_mut()
    }

    #[inline]
    pub fn insert(&self, key: ValueKey, value: Value) {
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
    pub fn add_fn(&self, id: &str, f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, false)));
    }

    #[inline]
    pub fn add_instance_fn(&self, id: &str, f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static) {
        self.add_value(id, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    #[inline]
    pub fn add_list(&self, id: &str, list: ValueList) {
        self.add_value(id, Value::List(list));
    }

    #[inline]
    pub fn add_map(&self, id: &str, map: ValueMap) {
        self.add_value(id, Value::Map(map));
    }

    #[inline]
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
            write!(f, "{}: {:#}", key.value(), value)?;
            first = false;
        }
        write!(f, "}}")
    }
}
