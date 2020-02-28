use crate::{
    runtime_error,
    value::{BuiltinResult, ExternalFunction},
    Error, RuntimeResult, Value,
};
use hashbrown::HashMap;
use koto_parser::{AstNode, Id, LookupId};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone)]
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

    pub fn add_list(&mut self, name: &str, list: Vec<Value<'a>>) {
        self.add_value(name, Value::List(Rc::new(list)));
    }

    pub fn add_map(&mut self, name: &str, map: ValueMap<'a>) {
        self.add_value(name, Value::Map(Rc::new(RefCell::new(map))));
    }

    pub fn add_value(&mut self, name: &str, value: Value<'a>) {
        self.0.insert(Rc::new(name.to_string()), value);
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
        id: &LookupId,
        id_index: usize,
        node: &AstNode,
        mut visitor: impl FnMut(&LookupId, &AstNode, &mut Value<'a>) -> RuntimeResult + 'a,
    ) -> (bool, RuntimeResult) {
        let id_first = id.0.first().unwrap().as_ref();
        if id.0.len() == 1 {
            match self.0.get_mut(id_first) {
                Some(mut value) => (true, visitor(id, node, &mut value)),
                _ => (false, runtime_error!(node, "Value not found: {}", id_first)),
            }
        } else {
            match self.0.get(id_first) {
                Some(Value::Map(map)) => {
                    map.borrow_mut().visit_mut(id, id_index + 1, node, visitor)
                }
                Some(unexpected) => (
                    false,
                    runtime_error!(node, "Expected map for {}, found {}", id_first, unexpected),
                ),
                _ => (false, runtime_error!(node, "Value not found: {}", id_first)),
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
