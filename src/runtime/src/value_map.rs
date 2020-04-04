use crate::{
    builtin_value::BuiltinValue,
    value::{make_builtin_value, BuiltinFunction},
    Id, RcCell, Runtime, RuntimeResult, Value, ValueList, BUILTIN_DATA_ID,
};
use rustc_hash::FxHashMap;
use std::{
    borrow::Borrow,
    cell::{Ref, RefMut},
    collections::hash_map::{Iter, Keys},
    hash::Hash,
};

#[derive(Clone, Debug, Default)]
pub struct ValueHashMap<'a>(FxHashMap<Id, Value<'a>>);

impl<'a> ValueHashMap<'a> {
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
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(id, Value::BuiltinFunction(BuiltinFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(id, Value::BuiltinFunction(BuiltinFunction::new(f, true)));
    }

    pub fn add_list(&mut self, id: &str, list: ValueList<'a>) {
        self.add_value(id, Value::List(list));
    }

    pub fn add_map(&mut self, id: &str, map: ValueMap<'a>) {
        self.add_value(id, Value::Map(map));
    }

    pub fn add_value(&mut self, id: &str, value: Value<'a>) {
        self.insert(Id::from_str(id), value);
    }

    pub fn insert(&mut self, id: Id, value: Value<'a>) {
        self.0.insert(id, value);
    }

    pub fn extend(&mut self, other: &ValueHashMap<'a>) {
        self.0.extend(other.0.clone().into_iter());
    }

    pub fn get<K: ?Sized>(&self, id: &K) -> Option<&Value<'a>>
    where
        Id: Borrow<K>,
        K: Hash + Eq,
    {
        self.0.get(id)
    }

    pub fn get_mut<K: ?Sized>(&mut self, id: &K) -> Option<&mut Value<'a>>
    where
        Id: Borrow<K>,
        K: Hash + Eq,
    {
        self.0.get_mut(id)
    }

    pub fn keys(&self) -> Keys<'_, Id, Value<'a>> {
        self.0.keys()
    }

    pub fn iter(&self) -> Iter<'_, Id, Value<'a>> {
        self.0.iter()
    }

    pub fn make_element_unique(&mut self, name: &str) -> Option<Value<'a>> {
        match self.0.get_mut(name) {
            Some(value) => {
                match value {
                    Value::Map(entry) => {
                        entry.make_unique();
                    }
                    Value::List(entry) => {
                        entry.make_unique();
                    }
                    _ => {}
                }
                Some(value.clone())
            }
            None => None,
        }
    }
}

impl<'a> PartialEq for ValueHashMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a> Eq for ValueHashMap<'a> {}

#[derive(Clone, Debug)]
pub struct ValueMap<'a>(RcCell<ValueHashMap<'a>>);

impl<'a> ValueMap<'a> {
    pub fn new() -> Self {
        Self(RcCell::new(ValueHashMap::default()))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(RcCell::new(ValueHashMap::with_capacity(capacity)))
    }

    pub fn with_data(data: ValueHashMap<'a>) -> Self {
        Self(RcCell::new(data))
    }

    pub fn data(&self) -> Ref<ValueHashMap<'a>> {
        self.0.borrow()
    }

    pub fn data_mut(&self) -> RefMut<ValueHashMap<'a>> {
        self.0.borrow_mut()
    }

    pub fn add_fn(
        &mut self,
        id: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(id, Value::BuiltinFunction(BuiltinFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        id: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(id, Value::BuiltinFunction(BuiltinFunction::new(f, true)));
    }

    pub fn add_list(&mut self, id: &str, list: ValueList<'a>) {
        self.add_value(id, Value::List(list));
    }

    pub fn add_map(&mut self, id: &str, map: ValueMap<'a>) {
        self.add_value(id, Value::Map(map));
    }

    pub fn add_value(&mut self, id: &str, value: Value<'a>) {
        self.insert(Id::from_str(id), value);
    }

    pub fn set_builtin_value(&mut self, data: impl BuiltinValue) {
        self.add_value(BUILTIN_DATA_ID, make_builtin_value(data));
    }

    pub fn insert(&mut self, name: Id, value: Value<'a>) {
        self.make_unique();
        self.data_mut().insert(name, value);
    }

    pub fn make_unique(&mut self) {
        self.0.make_unique();
    }

    pub fn make_element_unique(&self, name: &str) -> Option<Value<'a>> {
        self.data_mut().make_element_unique(name)
    }
}

impl<'a> PartialEq for ValueMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        *self.data() == *other.data()
    }
}
impl<'a> Eq for ValueMap<'a> {}

