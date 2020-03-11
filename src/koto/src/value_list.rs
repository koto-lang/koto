use crate::{
    runtime_error,
    value::{type_as_string, EvaluatedIndex, EvaluatedLookupNode},
    Error, LookupSlice, Value,
};
use koto_parser::AstNode;

use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub struct ValueList<'a>(Vec<Value<'a>>);

impl<'a> ValueList<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn with_data(data: Vec<Value<'a>>) -> Self {
        Self(data)
    }

    pub fn data(&self) -> &Vec<Value<'a>> {
        &self.0
    }

    pub fn data_mut(&mut self) -> &mut Vec<Value<'a>> {
        &mut self.0
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
            EvaluatedLookupNode::Index(list_index) => {
                if lookup_index == evaluated_lookup.len() - 1 {
                    match list_index {
                        EvaluatedIndex::Index(i) => {
                            if *i >= self.0.len() {
                                return runtime_error!(
                                    node,
                                    "Index out of bounds: \
                                     List in {} has a length of {} but the index is {}",
                                    lookup,
                                    self.0.len(),
                                    i
                                );
                            }
                            self.0[*i] = value.clone();
                        }
                        EvaluatedIndex::Range { min, max } => {
                            let umin = *min as usize;
                            let umax = *max as usize;
                            if *min < 0 || *max < 0 {
                                return runtime_error!(
                                    node,
                                    "Indexing with negative indices isn't supported, \
                                     min: {}, max: {}",
                                    min,
                                    max
                                );
                            } else if umin >= self.0.len() || umax > self.0.len() {
                                return runtime_error!(
                                    node,
                                    "Index out of bounds in '{}', \
                                     List has a length of {} - min: {}, max: {}",
                                    lookup,
                                    self.0.len(),
                                    min,
                                    max
                                );
                            } else {
                                for i in umin..umax {
                                    self.0[i] = value.clone();
                                }
                            }
                        }
                    }
                } else {
                    let list_index = match list_index {
                        EvaluatedIndex::Index(i) => {
                            if *i >= self.0.len() {
                                return runtime_error!(
                                    node,
                                    "Index out of bounds: \
                                     List in {} has a length of {} but the index is {}",
                                    lookup,
                                    self.0.len(),
                                    i
                                );
                            }
                            i
                        }
                        EvaluatedIndex::Range { .. } => {
                            return runtime_error!(
                                node,
                                "Ranges are only supported at the end of a lookup, in '{}'",
                                lookup
                            );
                        }
                    };

                    match &mut self.0[*list_index] {
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
                        Ref(ref_value) => match &mut *ref_value.borrow_mut() {
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
                        unexpected => {
                            return runtime_error!(
                                node,
                                "Expected List or Map in '{}', found {}",
                                lookup,
                                type_as_string(unexpected)
                            );
                        }
                    }
                }
            }
            EvaluatedLookupNode::Id(_) => {
                return runtime_error!(
                    node,
                    "Attempting to access a List like a Map in '{}'",
                    lookup
                );
            }
        }

        Ok(())
    }
}

impl<'a> fmt::Display for ValueList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, value) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", value)?;
        }
        write!(f, "]")
    }
}

impl<'a> PartialEq for ValueList<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
