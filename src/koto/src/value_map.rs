use crate::{
    runtime_error,
    value::{type_as_string, BuiltinResult, EvaluatedLookupNode, ExternalFunction},
    Error, LookupSlice, Value, ValueList,
};
use koto_parser::{AstNode, Id, LookupNode};
use rustc_hash::FxHashMap;
use std::{cell::RefCell, rc::Rc};

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

    pub fn add_fn(&mut self, name: &str, f: impl FnMut(&[Value<'a>]) -> BuiltinResult<'a> + 'a) {
        self.add_value(
            name,
            Value::ExternalFunction(ExternalFunction(Rc::new(RefCell::new(f)))),
        );
    }

    pub fn add_list(&mut self, name: &str, list: ValueList<'a>) {
        self.add_value(name, Value::List(Rc::new(list)));
    }

    pub fn add_map(&mut self, name: &str, map: ValueMap<'a>) {
        self.add_value(name, Value::Map(Rc::new(map)));
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
                        Value::Map(map) => map.visit(&id[1..], visitor),
                        _ => false,
                    }
                }
            }
            None => false,
        }
    }

    pub fn visit_mut<'b: 'a>(
        &mut self,
        id: &LookupSlice,
        id_index: usize,
        node: &AstNode,
        mut visitor: impl FnMut(&LookupSlice, &AstNode, &mut Value<'a>) -> Result<(), Error> + 'b,
    ) -> (bool, Result<(), Error>) {
        let entry_id = match &id.0[id_index] {
            LookupNode::Id(id) => id,
            _ => unreachable!(),
        };

        if id_index == id.0.len() - 1 {
            match self.0.get_mut(entry_id) {
                Some(mut value) => (true, visitor(id, node, &mut value)),
                _ => (false, runtime_error!(node, "Value not found: {}", entry_id)),
            }
        } else {
            match self.0.get_mut(entry_id) {
                Some(Value::Map(map)) => {
                    Rc::make_mut(map).visit_mut(id, id_index + 1, node, visitor)
                }
                list @ Some(Value::List(_)) => {
                    // todo
                    (true, visitor(id, node, &mut list.unwrap()))
                }
                Some(unexpected) => (
                    false,
                    runtime_error!(node, "Expected map for {}, found {}", entry_id, unexpected),
                ),
                _ => (false, runtime_error!(node, "Value not found: {}", entry_id)),
            }
        }
    }

    pub fn set_value_from_lookup(
        &mut self,
        lookup: &LookupSlice,
        evaluated_lookup: &[EvaluatedLookupNode],
        lookup_index: usize,
        value: &Value<'a>,
        node: &AstNode,
    ) -> Result<(), Error> {
        use Value::{List, Map, Ref};

        match &evaluated_lookup[lookup_index] {
            EvaluatedLookupNode::Id(id) => {
                if lookup_index == evaluated_lookup.len() - 1 {
                    self.0.insert(id.clone(), value.clone());
                } else {
                    match self.0.get_mut(id) {
                        Some(Map(entry)) => {
                            return Rc::make_mut(entry).set_value_from_lookup(
                                lookup,
                                evaluated_lookup,
                                lookup_index + 1,
                                value,
                                node,
                            );
                        }
                        Some(List(entry)) => {
                            return Rc::make_mut(entry).set_value_from_lookup(
                                lookup,
                                evaluated_lookup,
                                lookup_index + 1,
                                value,
                                node,
                            );
                        }
                        Some(Ref(ref_value)) => match &mut *ref_value.borrow_mut() {
                            Map(entry) => {
                                return Rc::make_mut(entry).set_value_from_lookup(
                                    lookup,
                                    evaluated_lookup,
                                    lookup_index + 1,
                                    value,
                                    node,
                                );
                            }
                            List(entry) => {
                                return Rc::make_mut(entry).set_value_from_lookup(
                                    lookup,
                                    evaluated_lookup,
                                    lookup_index + 1,
                                    value,
                                    node,
                                );
                            }
                            unexpected => {
                                return runtime_error!(
                                    node,
                                    "Expected List or Map in '{}', found {}",
                                    lookup,
                                    type_as_string(unexpected)
                                );
                            }
                        },
                        Some(unexpected) => {
                            return runtime_error!(
                                node,
                                "Expected List or Map in '{}', found {}",
                                lookup,
                                type_as_string(unexpected)
                            );
                        }
                        None => {
                            return runtime_error!(node, "'{}' not found in '{}'", id, lookup);
                        }
                    }
                }
            }
            EvaluatedLookupNode::Index(_) => {
                return runtime_error!(node, "Attempting to index a map in '{}'", lookup);
            }
        }

        Ok(())
    }
}

impl<'a> PartialEq for ValueMap<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a> Eq for ValueMap<'a> {}
