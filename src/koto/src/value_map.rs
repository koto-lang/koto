use crate::{
    runtime_error,
    value::{type_as_string, EvaluatedLookupNode, ExternalFunction},
    Error, LookupSlice, Runtime, RuntimeResult, Value, ValueList,
};
use koto_parser::{AstNode, Id};
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

    pub fn add_fn(
        &mut self,
        name: &str,
        f: impl FnMut(&mut Runtime<'a>, &[Value<'a>]) -> RuntimeResult<'a> + 'a,
    ) {
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
