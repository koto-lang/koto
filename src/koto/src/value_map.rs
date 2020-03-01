use crate::{
    runtime_error,
    value::{BuiltinResult, ExternalFunction},
    Error, LookupIdSlice, RuntimeResult, Value,
};
use rustc_hash::FxHashMap;
use koto_parser::{AstNode, Id};
use std::{cell::RefCell, rc::Rc};

pub type ValueHashMap<'a> = FxHashMap<Id, Value<'a>>;

#[derive(Debug, Clone)]
pub struct ValueMap<'a>(pub ValueHashMap<'a>);

impl<'a> ValueMap<'a> {
    pub fn new() -> Self {
        Self(ValueHashMap::default())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueHashMap::with_capacity_and_hasher(capacity, Default::default()))
    }

    pub fn add_fn(&mut self, name: &str, f: impl FnMut(&[Value<'a>]) -> BuiltinResult<'a> + 'a) {
        self.add_value(
            name,
            Value::ExternalFunction(ExternalFunction(Rc::new(RefCell::new(f)))),
        );
    }

    pub fn add_list(&mut self, name: &str, list: Vec<Value<'a>>) {
        self.add_value(name, Value::List(Rc::new(list)));
    }

    pub fn add_map(&mut self, name: &str, map: ValueMap<'a>) {
        self.add_value(name, Value::Map(Rc::new(RefCell::new(map))));
    }

    pub fn add_value(&mut self, name: &str, value: Value<'a>) {
        self.insert(Rc::new(name.to_string()), value);
    }

    pub fn insert(&mut self, name: Id, value: Value<'a>) {
        self.0.insert(name, value);
    }

    pub fn visit(&self, id: &[Id], visitor: impl Fn(&Value<'a>) + 'a) -> bool {
        match self.0.get(id.first().unwrap().as_ref()) {
            Some(value) => {
                if id.len() == 1 {
                    visitor(value);
                    true
                } else {
                    match value {
                        Value::Map(map) => map.borrow().visit(&id[1..], visitor),
                        _ => false,
                    }
                }
            }
            None => false,
        }
    }

    pub fn visit_mut(
        &mut self,
        id: &LookupIdSlice,
        id_index: usize,
        node: &AstNode,
        mut visitor: impl FnMut(&LookupIdSlice, &AstNode, &mut Value<'a>) -> RuntimeResult + 'a,
    ) -> (bool, RuntimeResult) {
        let entry_id = &id.0[id_index];
        if id_index == id.0.len() - 1 {
            match self.0.get_mut(entry_id) {
                Some(mut value) => (true, visitor(id, node, &mut value)),
                _ => (false, runtime_error!(node, "Value not found: {}", entry_id)),
            }
        } else {
            match self.0.get(entry_id) {
                Some(Value::Map(map)) => {
                    map.borrow_mut().visit_mut(id, id_index + 1, node, visitor)
                }
                Some(unexpected) => (
                    false,
                    runtime_error!(node, "Expected map for {}, found {}", entry_id, unexpected),
                ),
                _ => (false, runtime_error!(node, "Value not found: {}", entry_id)),
            }
        }
    }
}

impl<'a> PartialEq for ValueMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a> Eq for ValueMap<'a> {}
