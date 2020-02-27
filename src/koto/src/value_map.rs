use crate::{
    value::{BuiltinResult, ExternalFunction},
    Value,
};
use hashbrown::HashMap;
use koto_parser::Id;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct ValueMap<'a>(pub HashMap<Id, Value<'a>>);

impl<'a> ValueMap<'a> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn add_fn(&mut self, name: &str, f: impl FnMut(&[Value<'a>]) -> BuiltinResult<'a> + 'a) {
        self.add_value(
            name,
            Value::ExternalFunction(ExternalFunction(Rc::new(RefCell::new(f)))),
        );
    }

    pub fn add_map(&mut self, name: &str, map: ValueMap<'a>) {
        self.add_value(name, Value::Map(Rc::new(map)));
    }

    pub fn add_value(&mut self, name: &str, value: Value<'a>) {
        self.0.insert(Rc::new(name.to_string()), value);
    }
}

impl<'a> PartialEq for ValueMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a> Eq for ValueMap<'a> {}
