use crate::{value::BuiltinFunction, Id, RcCell, Runtime, RuntimeResult, Value, ValueList};
use rustc_hash::FxHashMap;

pub type ValueHashMap<'a> = FxHashMap<Id, Value<'a>>;

#[derive(Clone, Debug, Default)]
pub struct ValueMap<'a>(pub ValueHashMap<'a>);

impl<'a> ValueMap<'a> {
    pub fn new() -> Self {
        Self(ValueHashMap::default())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueHashMap::with_capacity_and_hasher(
            capacity,
            Default::default(),
        ))
    }

    pub fn add_fn(
        &mut self,
        name: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(name, Value::BuiltinFunction(BuiltinFunction::new(f, false)));
    }

    pub fn add_instance_fn(
        &mut self,
        name: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
        self.add_value(name, Value::BuiltinFunction(BuiltinFunction::new(f, true)));
    }

    pub fn add_list(&mut self, name: &str, list: ValueList<'a>) {
        self.add_value(name, Value::List(RcCell::new(list)));
    }

    pub fn add_map(&mut self, name: &str, map: ValueMap<'a>) {
        self.add_value(name, Value::Map(RcCell::new(map)));
    }

    pub fn add_value(&mut self, name: &str, value: Value<'a>) {
        self.insert(Id::new(name), value);
    }

    pub fn insert(&mut self, name: Id, value: Value<'a>) {
        self.0.insert(name, value);
    }

    pub fn make_unique(&mut self, name: &str) -> Option<Value<'a>> {
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

impl<'a> PartialEq for ValueMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a> Eq for ValueMap<'a> {}
