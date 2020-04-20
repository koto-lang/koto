use crate::{
    call_stack::CallStack,
    runtime_error,
    value::{copy_value, type_as_string, BuiltinFunction, RuntimeFunction, Value},
    value_iterator::{MultiRangeValueIterator, ValueIterator},
    value_list::ValueVec,
    Error, Id, LookupSlice, RuntimeResult, ValueHashMap, ValueList, ValueMap,
};
use koto_parser::{
    vec4, AssignTarget, AstFor, AstIf, AstNode, AstOp, AstWhile, ConstantIndex, ConstantPool,
    LookupNode, LookupOrId, LookupSliceOrId, Node, Scope,
};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::{fmt, rc::Rc, sync::Arc};

#[derive(Clone, Debug)]
pub enum ControlFlow<'a> {
    None,
    Function,
    Return,
    ReturnValue(Value<'a>),
    Loop,
    Break,
    Continue,
}

impl<'a> Default for ControlFlow<'a> {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
struct ValueAndLookupIndex<'a> {
    value: Value<'a>,
    lookup_index: Option<usize>,
}

impl<'a> ValueAndLookupIndex<'a> {
    fn new(value: Value<'a>, lookup_index: Option<usize>) -> Self {
        Self {
            value,
            lookup_index,
        }
    }
}

impl<'a> fmt::Display for ValueAndLookupIndex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Value: {}, LookupIndex: {:?}",
            self.value, self.lookup_index
        )
    }
}

#[derive(Debug)]
struct LookupResult<'a> {
    value: Value<'a>,
    parent: ValueAndLookupIndex<'a>,
}

impl<'a> LookupResult<'a> {
    fn new(value: Value<'a>, parent: ValueAndLookupIndex<'a>) -> Self {
        Self { value, parent }
    }
}

enum ValueOrValues<'a> {
    Value(Value<'a>),
    Values(ValueVec<'a>),
}

#[derive(Default)]
pub struct Runtime<'a> {
    constants: ConstantPool,
    string_constants: FxHashMap<u32, Arc<String>>,
    global: ValueHashMap<'a>,
    capture_map: Option<ValueMap<'a>>,
    call_stack: CallStack<'a>,
    control_flow: ControlFlow<'a>,
    script_path: Option<String>,
}

#[cfg(feature = "trace")]
#[macro_export]
macro_rules! runtime_trace  {
    ($self:expr, $message:expr) => {
        println!("{}{}", $self.runtime_indent(), $message);
    };
    ($self:expr, $message:expr, $($vals:expr),+) => {
        println!("{}{}", $self.runtime_indent(), format!($message, $($vals),+));
    };
}

#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! runtime_trace {
    ($self:expr, $message:expr) => {};
    ($self:expr, $message:expr, $($vals:expr),+) => {};
}

impl<'a> Runtime<'a> {
    pub fn new() -> Self {
        Self {
            global: ValueHashMap::with_capacity(32),
            control_flow: ControlFlow::None,
            call_stack: CallStack::with_capacity(32),
            script_path: None,
            ..Default::default()
        }
    }

    pub fn constants_mut(&mut self) -> &mut ConstantPool {
        &mut self.constants
    }

    pub fn global_mut(&mut self) -> &mut ValueHashMap<'a> {
        &mut self.global
    }

    pub fn set_script_path(&mut self, path: Option<String>) {
        self.script_path = path;
    }

    #[allow(dead_code)]
    fn runtime_indent(&self) -> String {
        "  ".repeat(self.call_stack.frame_count())
    }

    /// Evaluate a series of expressions
    pub fn evaluate_block(&mut self, block: &[AstNode]) -> RuntimeResult<'a> {
        use ControlFlow::*;

        runtime_trace!(self, "evaluate_block - {}", block.len());

        for expression in block.iter() {
            self.evaluate_and_expand(expression, false)?;
            match &self.control_flow {
                Return | Break | Continue => return Ok(Value::Empty),
                ReturnValue(result) => return Ok(result.clone()),
                _ => {}
            }
        }

        Ok(Value::Empty)
    }

    /// Evaluate a series of expressions and return the final result
    pub fn evaluate_block_and_capture(&mut self, block: &[AstNode]) -> RuntimeResult<'a> {
        use ControlFlow::*;

        runtime_trace!(self, "evaluate_block_and_capture - {}", block.len());

        for (i, expression) in block.iter().enumerate() {
            if i < block.len() - 1 {
                self.evaluate_and_expand(expression, false)?;
                match &self.control_flow {
                    Return | Break | Continue => return Ok(Value::Empty),
                    ReturnValue(result) => return Ok(result.clone()),
                    _ => {}
                }
            } else {
                let result = self.evaluate_and_capture(expression)?;

                return Ok(match &self.control_flow {
                    Return | Break | Continue => Value::Empty,
                    ReturnValue(result) => result.clone(),
                    _ => result,
                });
            }
        }

        Ok(Value::Empty)
    }

    /// Evaluate a series of expressions and capture their results in a list
    fn evaluate_expressions(
        &mut self,
        expressions: &[AstNode],
    ) -> Result<ValueOrValues<'a>, Error> {
        runtime_trace!(self, "evaluate_expressions: {}", expressions.len());

        if expressions.len() == 1 {
            Ok(ValueOrValues::Value(
                self.evaluate_and_capture(&expressions[0])?,
            ))
        } else {
            let results = expressions
                .iter()
                .map(|expression| {
                    Ok(if is_single_value_node(&expression.node) {
                        self.evaluate(expression)?
                    } else {
                        self.evaluate_and_capture(expression)?
                    })
                })
                .collect::<Result<ValueVec, Error>>()?;

            Ok(ValueOrValues::Values(results))
        }
    }

    /// Evaluate an expression and capture multiple return values in a List
    fn evaluate_and_capture(&mut self, expression: &AstNode) -> RuntimeResult<'a> {
        use Value::*;

        runtime_trace!(self, "evaluate_and_capture: {}", expression.node);

        if is_single_value_node(&expression.node) {
            self.evaluate(expression)
        } else {
            match self.evaluate_and_expand(expression, true)? {
                ValueOrValues::Value(value) => Ok(value),
                ValueOrValues::Values(values) => {
                    let list = values
                        .iter()
                        .cloned()
                        .map(|value| match value {
                            For(_) | Range { .. } => runtime_error!(
                                expression,
                                "Invalid value found in list capture: '{}'",
                                value
                            ),
                            _ => Ok(value),
                        })
                        .collect::<Result<ValueVec, Error>>()?;
                    Ok(List(ValueList::with_data(list)))
                }
            }
        }
    }

    /// Evaluates a single expression, and expands single return values
    ///
    /// A single For loop or Range in first position will be expanded
    fn evaluate_and_expand(
        &mut self,
        expression: &AstNode,
        capture: bool,
    ) -> Result<ValueOrValues<'a>, Error> {
        use Value::*;

        runtime_trace!(self, "evaluate_and_expand - {}", expression.node);

        let value = self.evaluate(expression)?;

        let expand_value = match value {
            For(_) | While(_) | Range { .. } => true,
            _ => false,
        };

        let result = if expand_value {
            match value {
                For(for_loop) => self.run_for_loop(&for_loop, expression, capture)?,
                While(while_loop) => self.run_while_loop(&while_loop, expression, capture)?,
                Range { start, end } => {
                    if capture {
                        let expanded = if end >= start {
                            (start..end).map(|n| Number(n as f64)).collect::<ValueVec>()
                        } else {
                            (end..start)
                                .rev()
                                .map(|n| Number(n as f64))
                                .collect::<ValueVec>()
                        };
                        List(ValueList::with_data(expanded))
                    } else {
                        Empty
                    }
                }
                _ => unreachable!(),
            }
        } else {
            value
        };

        Ok(ValueOrValues::Value(result))
    }

    pub fn evaluate(&mut self, node: &AstNode) -> RuntimeResult<'a> {
        runtime_trace!(self, "evaluate: {}", node.node);

        use Value::*;

        let result = match &node.node {
            Node::Empty => Empty,
            Node::BoolTrue => Bool(true),
            Node::BoolFalse => Bool(false),
            Node::Number(constant_index) => {
                Number(self.constants.get_f64(*constant_index as usize))
            }
            Node::Vec4(expressions) => self.make_vec4(expressions, node)?,
            Node::Str(constant_index) => Str(self.arc_string_from_constant(constant_index)),
            Node::List(elements) => match self.evaluate_expressions(elements)? {
                ValueOrValues::Value(value) => match value {
                    List(_) => value,
                    _ => List(ValueList::from_slice(&[value])),
                },
                ValueOrValues::Values(values) => List(ValueList::with_data(values)),
            },
            Node::Range {
                start,
                end,
                inclusive,
            } => self.make_range(start, end, *inclusive, node)?,
            Node::RangeFrom { start } => self.make_range_from(start, node)?,
            Node::RangeTo { end, inclusive } => self.make_range_to(end, *inclusive, node)?,
            Node::RangeFull => IndexRange {
                start: 0,
                end: None,
            },
            Node::Map(entries) => {
                let mut map = ValueHashMap::with_capacity(entries.len());
                for (id_index, node) in entries.iter() {
                    let value = self.evaluate_and_capture(node)?;
                    map.insert(self.id_from_constant(id_index), value);
                }
                Map(ValueMap::with_data(map))
            }
            Node::Lookup(lookup) => self.lookup_value_or_error(&lookup.as_slice(), node)?.value,
            Node::Id(id_index) => {
                self.get_value_or_error(self.get_constant_string(id_index), node)?
            }
            Node::Copy(lookup_or_id) => match lookup_or_id {
                LookupOrId::Id(id_index) => {
                    copy_value(&self.get_value_or_error(self.get_constant_string(id_index), node)?)
                }
                LookupOrId::Lookup(lookup) => {
                    copy_value(&self.lookup_value_or_error(&lookup.as_slice(), node)?.value)
                }
            },
            Node::CopyExpression(expression) => copy_value(&self.evaluate_and_capture(expression)?),
            Node::Block(block) => self.evaluate_block_and_capture(&block)?,
            Node::Expressions(expressions) => match self.evaluate_expressions(expressions)? {
                ValueOrValues::Value(value) => value,
                ValueOrValues::Values(values) => List(ValueList::with_data(values)),
            },
            Node::Return => match self.control_flow {
                ControlFlow::Function | ControlFlow::Loop => {
                    // TODO handle loop inside function
                    self.control_flow = ControlFlow::Return;
                    Value::Empty
                }
                _ => {
                    return runtime_error!(node, "'return' is only allowed inside a function");
                }
            },
            Node::ReturnExpression(expression) => match self.control_flow {
                ControlFlow::Function | ControlFlow::Loop => {
                    // TODO handle loop inside function
                    let value = self.evaluate_and_capture(expression)?;
                    self.control_flow = ControlFlow::ReturnValue(value.clone());
                    Value::Empty
                }
                _ => {
                    return runtime_error!(node, "'return' is only allowed inside a function");
                }
            },
            Node::Negate(expression) => {
                let value = self.evaluate_and_capture(expression)?;
                match value {
                    Bool(b) => Bool(!b),
                    Number(n) => Number(-n),
                    Vec4(v) => Vec4(-v),
                    unexpected => {
                        return runtime_error!(
                            node,
                            "Expected negatable value, found {}",
                            unexpected
                        );
                    }
                }
            }
            Node::Function(f) => {
                let function = f.clone();
                let captured = function
                    .captures
                    .iter()
                    .map(|capture_index| {
                        let id = self.id_from_constant(capture_index);
                        Ok((id.clone(), self.get_value_or_error(id.as_str(), node)?))
                    })
                    .collect::<Result<ValueHashMap, Error>>()?;
                Function(RuntimeFunction {
                    function,
                    captured: ValueMap::with_data(captured),
                })
            }
            Node::Call { function, args } => {
                self.lookup_and_call_function(&function.as_slice(), args, node)?
            }
            Node::Debug { expressions } => self.debug_statement(expressions, node)?,
            Node::Assign { target, expression } => self.assign_value(target, expression, node)?,
            Node::MultiAssign {
                targets,
                expressions,
            } => self.assign_values(targets, expressions, node)?,
            Node::Op { op, lhs, rhs } => self.binary_op(op, lhs, rhs, node)?,
            Node::If(if_statement) => self.do_if_statement(if_statement, node)?,
            Node::For(for_statement) => For(for_statement.clone()),
            Node::While(while_loop) => While(while_loop.clone()),
            Node::Break => {
                if !matches!(self.control_flow, ControlFlow::Loop) {
                    return runtime_error!(node, "'break' found outside of loop");
                }
                self.control_flow = ControlFlow::Break;
                Empty
            }
            Node::Continue => {
                if !matches!(self.control_flow, ControlFlow::Loop) {
                    return runtime_error!(node, "'continue' found outside of loop");
                }
                self.control_flow = ControlFlow::Continue;
                Empty
            }
        };

        Ok(result)
    }

    fn set_value(&mut self, id: &Id, value: Value<'a>, scope: Scope) {
        runtime_trace!(self, "set_value - {}: {} - {:?}", id, value, scope);

        if self.call_stack.frame_count() == 0 || scope == Scope::Global {
            match self.global.get_mut(id) {
                Some(exists) => {
                    *exists = value;
                }
                None => {
                    self.global.insert(id.clone(), value);
                }
            }
        } else {
            if let Some(capture_map) = &self.capture_map {
                if let Some(exists) = capture_map.data_mut().get_mut(id.as_str()) {
                    *exists = value;
                    return;
                }
            }

            match self.call_stack.get_mut(id.as_str()) {
                Some(exists) => {
                    *exists = value;
                }
                None => {
                    self.call_stack.extend_frame(id.clone(), value);
                }
            }
        }
    }

    pub fn get_value(&self, id: &str) -> Option<Value<'a>> {
        runtime_trace!(self, "get_value: {}", id);

        if self.call_stack.frame_count() > 0 {
            if let Some(value) = self.call_stack.get(id) {
                return Some(value.clone());
            }

            if let Some(capture_map) = &self.capture_map {
                if let Some(value) = capture_map.data().get(id) {
                    return Some(value.clone());
                }
            }
        }

        match self.global.get(id) {
            Some(value) => Some(value.clone()),
            None => None,
        }
    }

    fn get_value_or_error(&self, id: &str, node: &AstNode) -> Result<Value<'a>, Error> {
        match self.get_value(id) {
            Some(v) => Ok(v),
            None => runtime_error!(node, "'{}' not found", id),
        }
    }

    fn set_value_from_lookup(
        &mut self,
        lookup: &LookupSlice,
        value: Value<'a>,
        node: &AstNode,
    ) -> Result<(), Error> {
        let root_id = match &lookup.0[0] {
            LookupNode::Id(id_index) => self.id_from_constant(id_index),
            _ => unreachable!(),
        };

        if self.call_stack.frame_count() > 0 {
            let maybe_root = {
                if let Some(root) = self.call_stack.get(root_id.as_str()).cloned() {
                    Some(root)
                } else if let Some(capture_map) = &self.capture_map {
                    capture_map.data().get(root_id.as_str()).cloned()
                } else {
                    None
                }
            };

            if let Some(root) = maybe_root {
                let root = root.clone();
                self.do_lookup(lookup, root, Some(value), node)?;
                return Ok(());
            }
        }

        if let Some(root) = self.global.get(root_id.as_str()) {
            let root = root.clone();
            self.do_lookup(lookup, root, Some(value), node)?;
            return Ok(());
        }

        runtime_error!(node, "'{}' not found", root_id)
    }

    fn do_lookup(
        &mut self,
        lookup: &LookupSlice,
        root: Value<'a>,
        value_to_set: Option<Value<'a>>,
        node: &AstNode,
    ) -> Result<Option<(Value<'a>, ValueAndLookupIndex<'a>)>, Error> {
        use Value::{BuiltinFunction, Function, IndexRange, List, Map, Number, Range};

        runtime_trace!(
            self,
            "do_lookup: {} - root: {} - value_to_set",
            lookup,
            root,
            if value_to_set.is_some() {
                value_to_set.unwrap().to_string()
            } else {
                "None".to_string()
            }
        );
        assert!(lookup.0.len() > 1);

        let mut parent = ValueAndLookupIndex::new(root.clone(), Some(0));
        let mut current_node = root;
        let mut temporary_value = false;

        for (lookup_index, lookup_node) in lookup.0[1..].iter().enumerate() {
            // We want to keep track of the parent container for the next lookup node
            // If the current node is a function then we skip over it
            parent = match &current_node {
                Function { .. } | BuiltinFunction(_) => parent,
                _ => ValueAndLookupIndex::new(
                    current_node.clone(),
                    if temporary_value {
                        None
                    } else {
                        Some(lookup_index)
                    },
                ),
            };

            match current_node.clone() {
                Map(map) => match lookup_node {
                    LookupNode::Id(id_index) => {
                        let id = self.get_constant_string(id_index);
                        match &value_to_set {
                            Some(value) => {
                                if (lookup_index + 1) == lookup.0.len() - 1 {
                                    map.data_mut()
                                        .insert(self.id_from_constant(id_index), value.clone());
                                    return Ok(None);
                                } else {
                                    match map.data().get(id).clone() {
                                        Some(child_node) => {
                                            current_node = child_node.clone();
                                        }
                                        None => {
                                            return runtime_error!(
                                                node,
                                                "'{}' not found in '{}'",
                                                id,
                                                self.lookup_slice_to_string(lookup)
                                            );
                                        }
                                    }
                                }
                            }
                            None => match map.data().get(id) {
                                Some(value) => current_node = value.clone(),
                                None => {
                                    return Ok(None);
                                }
                            },
                        }
                    }
                    LookupNode::Index(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to index a Map in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                    LookupNode::Call(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to call a Map like a Function in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                },
                List(list) => match lookup_node {
                    LookupNode::Index(index) => {
                        let list_len = list.data().len();
                        match self.evaluate(&index.0)? {
                            Number(i) => {
                                let i = i as usize;
                                if i < list_len {
                                    match &value_to_set {
                                        Some(value) => {
                                            if (lookup_index + 1) == lookup.0.len() - 1 {
                                                list.data_mut()[i] = value.clone();
                                                return Ok(None);
                                            } else {
                                                current_node = list.data()[i].clone();
                                            }
                                        }
                                        None => {
                                            current_node = list.data()[i].clone();
                                        }
                                    }
                                } else {
                                    return runtime_error!(
                                        node,
                                        "Index out of bounds in '{}', \
                                         List has a length of {} but the index is {}",
                                        self.lookup_slice_to_string(lookup),
                                        list_len,
                                        i
                                    );
                                }
                            }
                            Range { start, end } => {
                                let ustart = start as usize;
                                let uend = end as usize;

                                if (lookup_index + 1) < (lookup.0.len() - 1) {
                                    return runtime_error!(
                                        node,
                                        "Indexing with a range is only supported at the end of a \
                                         lookup chain (in '{}')",
                                        self.lookup_slice_to_string(lookup)
                                    );
                                } else if start < 0 || end < 0 {
                                    return runtime_error!(
                                        node,
                                        "Indexing with negative indices isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if start > end {
                                    return runtime_error!(
                                        node,
                                        "Indexing with a descending range isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if ustart > list_len || uend > list_len {
                                    return runtime_error!(
                                        node,
                                        "Index out of bounds in '{}', \
                                         List has a length of {} - start: {}, end: {}",
                                        self.lookup_slice_to_string(lookup),
                                        list_len,
                                        start,
                                        end
                                    );
                                } else {
                                    match &value_to_set {
                                        Some(value) => {
                                            let mut list_data = list.data_mut();
                                            for i in ustart..uend {
                                                list_data[i] = value.clone();
                                            }
                                            return Ok(None);
                                        }
                                        None => {
                                            // TODO Avoid allocating new vec,
                                            // introduce 'slice' value type
                                            current_node = List(ValueList::from_slice(
                                                &list.data()[ustart..uend],
                                            ))
                                        }
                                    }
                                }
                            }
                            IndexRange { start, end } => {
                                let end = end.unwrap_or_else(|| list_len);

                                if (lookup_index + 1) < (lookup.0.len() - 1) {
                                    return runtime_error!(
                                        node,
                                        "Indexing with a range is only supported at the end of a \
                                         lookup chain (in '{}')",
                                        self.lookup_slice_to_string(lookup)
                                    );
                                } else if start > end {
                                    return runtime_error!(
                                        node,
                                        "Indexing with a descending range isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if start > list_len || end > list_len {
                                    return runtime_error!(
                                        node,
                                        "Index out of bounds in '{}', \
                                         List has a length of {} - start: {}, end: {}",
                                        self.lookup_slice_to_string(lookup),
                                        list_len,
                                        start,
                                        end
                                    );
                                } else {
                                    match &value_to_set {
                                        Some(value) => {
                                            let mut list_data = list.data_mut();
                                            for i in start..end {
                                                list_data[i] = value.clone();
                                            }
                                        }
                                        None => {
                                            // TODO Avoid allocating new vec,
                                            // introduce 'slice' value type
                                            current_node = List(ValueList::from_slice(
                                                &list.data()[start..end],
                                            ))
                                        }
                                    }
                                }
                            }
                            unexpected => {
                                return runtime_error!(
                                    node,
                                    "Indexing is only supported with number values or ranges, \
                                     found {})",
                                    type_as_string(&unexpected)
                                )
                            }
                        };
                    }
                    LookupNode::Id(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to access a List like a map in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                    LookupNode::Call(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to call a List like a Function in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                },
                Function(function) => match lookup_node {
                    LookupNode::Call(args) => {
                        temporary_value = true;

                        current_node = self.evaluate_args_and_call_function(
                            &function,
                            &LookupSliceOrId::LookupSlice(lookup.first_n(lookup_index)),
                            Some(parent.clone()),
                            args,
                            node,
                        )?;
                    }
                    LookupNode::Id(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to access a Function like a Map in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                    LookupNode::Index(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to index a Function in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                },
                BuiltinFunction(function) => match lookup_node {
                    LookupNode::Call(args) => {
                        temporary_value = true;

                        current_node = self.call_builtin_function(
                            &function,
                            &LookupSliceOrId::LookupSlice(lookup.first_n(lookup_index)),
                            Some(parent.clone()),
                            args,
                            node,
                        )?;
                    }
                    LookupNode::Id(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to access a Function like a Map in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                    LookupNode::Index(_) => {
                        return runtime_error!(
                            node,
                            "Attempting to index a Function in '{}'",
                            self.lookup_slice_to_string(lookup)
                        );
                    }
                },
                unexpected => {
                    return runtime_error!(
                        node,
                        "Unexpected type encountered during lookup: '{}'",
                        type_as_string(&unexpected)
                    )
                }
            }
        }

        Ok(Some((current_node, parent)))
    }

    fn lookup_value(
        &mut self,
        lookup: &LookupSlice,
        node: &AstNode,
    ) -> Result<Option<LookupResult<'a>>, Error> {
        runtime_trace!(self, "lookup_value: {}", lookup);

        let root_id = match &lookup.0[0] {
            LookupNode::Id(id_index) => self.id_from_constant(id_index),
            _ => unreachable!(),
        };

        if self.call_stack.frame_count() > 0 {
            let maybe_root = {
                if let Some(root) = self.call_stack.get(root_id.as_str()).cloned() {
                    Some(root)
                } else if let Some(capture_map) = &self.capture_map {
                    capture_map.data().get(root_id.as_str()).cloned()
                } else {
                    None
                }
            };

            if let Some(root) = maybe_root {
                if let Some((found, parent)) = self.do_lookup(lookup, root, None, node)? {
                    return Ok(Some(LookupResult::new(found, parent)));
                }
            }
        }

        match self.global.get(root_id.as_str()).cloned() {
            Some(root) => match self.do_lookup(lookup, root, None, node)? {
                Some((found, parent)) => Ok(Some(LookupResult::new(found, parent))),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    fn lookup_value_or_error(
        &mut self,
        id: &LookupSlice,
        node: &AstNode,
    ) -> Result<LookupResult<'a>, Error> {
        match self.lookup_value(id, node)? {
            Some(v) => Ok(v),
            None => runtime_error!(node, "'{}' not found", self.lookup_slice_to_string(id)),
        }
    }

    fn run_for_loop(
        &mut self,
        for_loop: &Rc<AstFor>,
        node: &AstNode,
        capture: bool,
    ) -> RuntimeResult<'a> {
        runtime_trace!(self, "run_for_loop");
        use Value::*;

        let f = &for_loop;

        let mut captured = ValueVec::new();

        if f.ranges.len() == 1 {
            let range = self.evaluate(f.ranges.first().unwrap())?;

            let value_iter = match range {
                v @ List(_) | v @ Range { .. } => Ok(ValueIterator::new(v)),
                unexpected => runtime_error!(
                    node,
                    "Expected iterable range in for statement, found {}",
                    type_as_string(&unexpected)
                ),
            }?;

            let single_arg = f.args.len() == 1;
            let first_arg = f.args.first().unwrap();

            for value in value_iter {
                if single_arg {
                    let id = self.id_from_constant(first_arg);
                    self.set_value(&id, value.clone(), Scope::Local);
                } else {
                    let mut arg_iter = f.args.iter().peekable();
                    match value {
                        List(a) => {
                            for list_value in a.data().iter() {
                                match arg_iter.next() {
                                    Some(arg) => {
                                        let id = Id::new(self.arc_string_from_constant(arg));
                                        self.set_value(&id, list_value.clone(), Scope::Local)
                                    }
                                    None => break,
                                }
                            }
                        }
                        _ => {
                            let id = self.id_from_constant(
                                arg_iter
                                    .next()
                                    .expect("For loops have at least one argument"),
                            );
                            self.set_value(&id, value.clone(), Scope::Local)
                        }
                    }
                    for remaining_arg in arg_iter {
                        let id = self.id_from_constant(remaining_arg);
                        self.set_value(&id, Value::Empty, Scope::Local);
                    }
                }

                if let Some(condition) = &f.condition {
                    let condition_result = self.evaluate(&condition)?;

                    match condition_result {
                        Bool(b) => {
                            if !b {
                                continue;
                            }
                        }
                        unexpected => {
                            return runtime_error!(
                                node,
                                "Expected bool in for statement condition, found {}",
                                type_as_string(&unexpected)
                            )
                        }
                    }
                }

                let cached_control_flow = self.control_flow.clone();
                self.control_flow = ControlFlow::Loop;

                let result = self.evaluate_and_capture(&f.body)?;

                match self.control_flow {
                    ControlFlow::Return | ControlFlow::ReturnValue(_) => return Ok(Empty),
                    ControlFlow::Break => {
                        self.control_flow = cached_control_flow;
                        break;
                    }
                    ControlFlow::Continue => {
                        self.control_flow = cached_control_flow;
                        continue;
                    }
                    _ => {
                        self.control_flow = cached_control_flow;
                    }
                }

                if capture {
                    captured.push(result);
                }
            }
        } else {
            let mut multi_range_iterator = MultiRangeValueIterator::with_capacity(f.ranges.len());
            for range in f.ranges.iter() {
                let range = self.evaluate(range)?;

                match range {
                    v @ List(_) | v @ Range { .. } => {
                        multi_range_iterator.iterators.push(ValueIterator::new(v))
                    }
                    unexpected => {
                        return runtime_error!(
                            node,
                            "Expected iterable range in for statement, found {}",
                            type_as_string(&unexpected)
                        )
                    }
                }
            }

            let single_arg = f.args.len() == 1;
            let first_arg = f.args.first().unwrap();

            let mut values = ValueVec::new();

            while multi_range_iterator.get_next_values(&mut values) {
                if single_arg {
                    if values.len() == 1 {
                        let id = self.id_from_constant(first_arg);
                        self.set_value(&id, values[0].clone(), Scope::Local);
                    } else {
                        let id = self.id_from_constant(first_arg);
                        self.set_value(
                            &id,
                            List(ValueList::with_data(values.clone())),
                            Scope::Local,
                        );
                    }
                } else {
                    let mut arg_iter = f.args.iter().peekable();
                    for value in values.iter() {
                        match arg_iter.next() {
                            Some(arg) => {
                                let id = self.id_from_constant(arg);
                                self.set_value(&id, value.clone(), Scope::Local);
                            }
                            None => break,
                        }
                    }
                    for remaining_arg in arg_iter {
                        let id = self.id_from_constant(remaining_arg);
                        self.set_value(&id, Value::Empty, Scope::Local);
                    }
                }

                if let Some(condition) = &f.condition {
                    match self.evaluate(&condition)? {
                        Bool(b) => {
                            if !b {
                                continue;
                            }
                        }
                        unexpected => {
                            return runtime_error!(
                                node,
                                "Expected bool in for statement condition, found {}",
                                type_as_string(&unexpected)
                            )
                        }
                    }
                }

                let cached_control_flow = self.control_flow.clone();
                self.control_flow = ControlFlow::Loop;

                let result = self.evaluate_and_capture(&f.body)?;

                match self.control_flow {
                    ControlFlow::Return | ControlFlow::ReturnValue(_) => return Ok(Empty),
                    ControlFlow::Break => {
                        self.control_flow = cached_control_flow;
                        break;
                    }
                    ControlFlow::Continue => {
                        self.control_flow = cached_control_flow;
                        continue;
                    }
                    _ => {
                        self.control_flow = cached_control_flow;
                    }
                }

                if capture {
                    captured.push(result);
                }
            }
        }

        Ok(if captured.is_empty() {
            Empty
        } else {
            List(ValueList::with_data(captured))
        })
    }

    fn run_while_loop(
        &mut self,
        while_loop: &Rc<AstWhile>,
        node: &AstNode,
        capture: bool,
    ) -> RuntimeResult<'a> {
        use Value::{Bool, Empty, List};

        runtime_trace!(self, "run_while_loop");

        let mut captured = ValueVec::new();
        loop {
            match self.evaluate(&while_loop.condition)? {
                Bool(condition_result) => {
                    if condition_result != while_loop.negate_condition {
                        let cached_control_flow = self.control_flow.clone();
                        self.control_flow = ControlFlow::Loop;

                        let result = self.evaluate_and_capture(&while_loop.body)?;

                        use ControlFlow::*;
                        match self.control_flow {
                            Break => {
                                self.control_flow = cached_control_flow;
                                break;
                            }
                            Continue => {
                                self.control_flow = cached_control_flow;
                                continue;
                            }
                            Return | ReturnValue(_) => return Ok(Empty),
                            _ => {
                                self.control_flow = cached_control_flow;
                            }
                        }

                        if capture {
                            captured.push(result);
                        };
                    } else {
                        break;
                    }
                }
                unexpected => {
                    return runtime_error!(
                        node,
                        "Expected bool in while condition, found '{}'",
                        unexpected
                    );
                }
            }
        }

        Ok(if captured.is_empty() {
            Empty
        } else {
            List(ValueList::with_data(captured))
        })
    }

    pub fn debug_statement(
        &mut self,
        expressions: &[(ConstantIndex, AstNode)],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        let prefix = match &self.script_path {
            Some(path) => format!("[{}: {}]", path, node.start_pos.line),
            None => format!("[{}]", node.start_pos.line),
        };
        for (debug_text_id, expression) in expressions.iter() {
            let value = self.evaluate_and_capture(expression)?;
            println!(
                "{} {}: {}",
                prefix,
                self.get_constant_string(debug_text_id),
                value
            );
        }
        Ok(Value::Empty)
    }

    pub fn lookup_and_call_function(
        &mut self,
        lookup_or_id: &LookupSliceOrId,
        args: &[AstNode],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::*;

        runtime_trace!(self, "lookup_and_call_function - {}", function_id);

        let (maybe_function, maybe_parent) = match lookup_or_id {
            LookupSliceOrId::Id(id_index) => {
                (self.get_value(self.get_constant_string(id_index)), None)
            }
            LookupSliceOrId::LookupSlice(lookup) => match self.lookup_value(&lookup, node)? {
                Some(lookup_result) => (Some(lookup_result.value), Some(lookup_result.parent)),
                None => (None, None),
            },
        };

        match maybe_function {
            Some(BuiltinFunction(f)) => {
                self.call_builtin_function(&f, lookup_or_id, maybe_parent, args, node)
            }
            Some(Function(f)) => {
                self.evaluate_args_and_call_function(&f, lookup_or_id, maybe_parent, args, node)
            }
            Some(unexpected) => runtime_error!(
                node,
                "Expected '{}' to be a Function, found {}",
                self.lookup_slice_or_id_to_string(lookup_or_id),
                type_as_string(&unexpected)
            ),
            None => runtime_error!(
                node,
                "Function '{}' not found",
                self.lookup_slice_or_id_to_string(lookup_or_id)
            ),
        }
    }

    fn call_builtin_function(
        &mut self,
        builtin: &BuiltinFunction<'a>,
        lookup_or_id: &LookupSliceOrId,
        parent: Option<ValueAndLookupIndex<'a>>,
        args: &[AstNode],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        runtime_trace!(
            self,
            "call_builtin_function - {} - parent: {:?}",
            lookup_or_id,
            parent
        );

        let evaluated_args = self.evaluate_expressions(args)?;

        let mut builtin_function = builtin.function.borrow_mut();

        let builtin_result = if builtin.is_instance_function {
            match parent {
                Some(parent) => match evaluated_args {
                    ValueOrValues::Value(value) => {
                        (&mut *builtin_function)(self, &[parent.value, value])
                    }
                    ValueOrValues::Values(mut values) => {
                        values.insert(0, parent.value);
                        (&mut *builtin_function)(self, &values)
                    }
                },
                None => {
                    return runtime_error!(
                        node,
                        "External instance function '{}' can only be called if contained in a Map",
                        self.lookup_slice_or_id_to_string(lookup_or_id)
                    );
                }
            }
        } else {
            match evaluated_args {
                ValueOrValues::Value(value) => (&mut *builtin_function)(self, &[value]),
                ValueOrValues::Values(values) => (&mut *builtin_function)(self, &values),
            }
        };

        match builtin_result {
            Err(Error::BuiltinError { message }) => runtime_error!(node, message),
            other => other,
        }
    }

    fn evaluate_args_and_call_function(
        &mut self,
        f: &RuntimeFunction<'a>,
        lookup_or_id: &LookupSliceOrId,
        parent: Option<ValueAndLookupIndex<'a>>,
        args: &[AstNode],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        runtime_trace!(
            self,
            "evaluate_args_and_call_function - {} - parent: {}",
            lookup_or_id,
            if parent.is_some() {
                format!("{}", parent.clone().unwrap())
            } else {
                "None".to_string()
            }
        );

        let mut call_frame: SmallVec<[(Id, Value<'a>); 8]> = SmallVec::new();

        let implicit_self = match lookup_or_id {
            LookupSliceOrId::Id(id_index) => {
                // allow standalone functions to be able to call themselves
                let id = self.id_from_constant(id_index);
                call_frame.push((id, Value::Function(f.clone())));
                false
            }
            LookupSliceOrId::LookupSlice(_) => {
                // implicit self for map functions
                match f.function.args.first() {
                    Some(arg_index) => {
                        if self.get_constant_string(arg_index) == "self" {
                            let id = self.id_from_constant(arg_index);
                            call_frame.push((id, parent.unwrap().value));
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
        };

        let function_args = &f.function.args;

        let arg_count = function_args.len();
        let expected_args = if implicit_self {
            arg_count - 1
        } else {
            arg_count
        };

        if args.len() != expected_args {
            return runtime_error!(
                node,
                "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                self.lookup_slice_or_id_to_string(lookup_or_id),
                expected_args,
                args.len(),
                function_args
            );
        }

        for (name_index, arg) in function_args
            .iter()
            .skip(if implicit_self { 1 } else { 0 })
            .zip(args.iter())
        {
            let arg_value = match self.evaluate_and_capture(arg) {
                Ok(value) => value,
                e @ Err(_) => {
                    return e;
                }
            };

            let id = self.id_from_constant(name_index);
            call_frame.push((id, arg_value));
        }

        self.call_function(f, &call_frame)
    }

    pub fn call_function_with_evaluated_args(
        &mut self,
        f: &RuntimeFunction<'a>,
        args: &[Value<'a>],
    ) -> RuntimeResult<'a> {
        if f.function.args.len() != args.len() {
            return runtime_error!(
                f.function
                    .body
                    .first()
                    .expect("A function must have at least one node in its body"),
                "Mismatch in number of arguments when calling function, expected {}, found {}",
                f.function.args.len(),
                args.len()
            );
        }

        let mut call_frame: SmallVec<[(Id, Value<'a>); 8]> = SmallVec::new();

        for (name_index, arg) in f.function.args.iter().zip(args.iter()) {
            let id = self.id_from_constant(name_index);
            call_frame.push((id, arg.clone()));
        }

        self.call_function(f, &call_frame)
    }

    pub fn call_function(
        &mut self,
        f: &RuntimeFunction<'a>,
        args: &[(Id, Value<'a>)],
    ) -> RuntimeResult<'a> {
        self.call_stack.push_frame(&args);
        let cached_control_flow = self.control_flow.clone();
        self.control_flow = ControlFlow::Function;
        let cached_capture_map = self.capture_map.clone();
        self.capture_map = Some(f.captured.clone());

        let mut result = self.evaluate_block(&f.function.body);

        if let ControlFlow::ReturnValue(return_value) = &self.control_flow {
            result = Ok(return_value.clone());
        }

        self.capture_map = cached_capture_map;
        self.control_flow = cached_control_flow;
        self.call_stack.pop_frame();

        result
    }

    fn assign_value(
        &mut self,
        target: &AssignTarget,
        expression: &AstNode,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        let value = self.evaluate_and_capture(expression)?;

        match target {
            AssignTarget::Id { id_index, scope } => {
                let id = self.id_from_constant(id_index);
                self.set_value(&id, value.clone(), *scope);
            }
            AssignTarget::Lookup(lookup) => {
                self.set_value_from_lookup(&lookup.as_slice(), value.clone(), node)?;
            }
        }

        Ok(value)
    }

    fn assign_values(
        &mut self,
        targets: &[AssignTarget],
        expressions: &[AstNode],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::{Empty, List};

        macro_rules! set_value {
            ($target:expr, $value:expr) => {
                match $target {
                    AssignTarget::Id { id_index, scope } => {
                        let id = self.id_from_constant(id_index);
                        self.set_value(&id, $value, *scope);
                    }
                    AssignTarget::Lookup(lookup) => {
                        self.set_value_from_lookup(&lookup.as_slice(), $value, node)?;
                    }
                }
            };
        };

        if expressions.len() == 1 {
            let value = self.evaluate_and_capture(&expressions[0])?;

            match &value {
                List(l) => {
                    let list_data = l.data();
                    let mut result_iter = list_data.iter();
                    for target in targets.iter() {
                        let value = result_iter.next().unwrap_or(&Empty);
                        set_value!(target, value.clone());
                    }
                }
                _ => {
                    let first_id = targets.first().unwrap();
                    runtime_trace!(
                        self,
                        "Assigning to {}: {} (single expression)",
                        first_id,
                        value
                    );
                    set_value!(first_id, value.clone());

                    for id in targets[1..].iter() {
                        set_value!(id, Empty);
                    }
                }
            }

            Ok(value)
        } else {
            let mut results = ValueVec::new();

            for expression in expressions.iter() {
                let value = self.evaluate_and_capture(expression)?;
                results.push(value);
            }

            match results.as_slice() {
                [] => unreachable!(),
                [single_value] => {
                    let first_id = targets.first().unwrap();
                    runtime_trace!(self, "Assigning to {}: {}", first_id, single_value);
                    set_value!(first_id, single_value.clone());
                    // set remaining targets to empty
                    for id in targets[1..].iter() {
                        runtime_trace!(self, "Assigning to {}: ()");
                        set_value!(id, Empty);
                    }
                }
                _ => {
                    let mut result_iter = results.iter();
                    for id in targets.iter() {
                        let value = result_iter.next().unwrap_or(&Empty).clone();
                        runtime_trace!(self, "Assigning to {}: {}", id, value);
                        set_value!(id, value);
                    }
                }
            }

            // TODO This capture only needs to take place when its the final statement in a
            //      block, e.g. last statement in a function
            Ok(List(ValueList::with_data(results)))
        }
    }

    fn make_range(
        &mut self,
        start: &AstNode,
        end: &AstNode,
        inclusive: bool,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::{Number, Range};

        let start = self.evaluate(start)?;
        let end = self.evaluate(end)?;

        match (start, end) {
            (Number(start), Number(end)) => {
                let start = start as isize;
                let end = end as isize;

                let (start, end) = if start <= end {
                    if inclusive {
                        (start, end + 1)
                    } else {
                        (start, end)
                    }
                } else {
                    // descending ranges will be evaluated with (end..start).rev()
                    if inclusive {
                        (start + 1, end)
                    } else {
                        (start + 1, end + 1)
                    }
                };

                Ok(Range { start, end })
            }
            unexpected => {
                return runtime_error!(
                    node,
                    "Expected numbers for range bounds, found start: {}, end: {}",
                    type_as_string(&unexpected.0),
                    type_as_string(&unexpected.1)
                )
            }
        }
    }

    fn make_range_from(
        &mut self,
        start_expression: &Box<AstNode>,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::{IndexRange, Number};

        let evaluated_start = match self.evaluate(start_expression)? {
            Number(n) => {
                if n < 0.0 {
                    return runtime_error!(
                        node,
                        "Negative numbers aren't allowed in index ranges, found {}",
                        n
                    );
                }

                n as usize
            }
            unexpected => {
                return runtime_error!(
                    node,
                    "Expected Number for range start, found '{}'",
                    type_as_string(&unexpected)
                );
            }
        };

        Ok(IndexRange {
            start: evaluated_start,
            end: None,
        })
    }

    fn make_range_to(
        &mut self,
        end_expression: &Box<AstNode>,
        inclusive: bool,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::{IndexRange, Number};

        let evaluated_end = match self.evaluate(end_expression)? {
            Number(n) => {
                if n < 0.0 {
                    return runtime_error!(
                        node,
                        "Negative numbers aren't allowed in index ranges, found {}",
                        n
                    );
                }

                if inclusive {
                    n as usize + 1
                } else {
                    n as usize
                }
            }
            unexpected => {
                return runtime_error!(
                    node,
                    "Expected Number for range end, found '{}'",
                    type_as_string(&unexpected)
                );
            }
        };

        Ok(IndexRange {
            start: 0,
            end: Some(evaluated_end),
        })
    }

    fn binary_op(
        &mut self,
        op: &AstOp,
        lhs: &AstNode,
        rhs: &AstNode,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        runtime_trace!(self, "binary_op: {:?}", op);

        use Value::*;

        let binary_op_error = |lhs, rhs| {
            runtime_error!(
                node,
                "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
                op,
                lhs,
                rhs
            )
        };

        let lhs_value = self.evaluate_and_capture(lhs)?;

        match op {
            AstOp::And => {
                return if let Bool(a) = lhs_value {
                    if a {
                        match self.evaluate_and_capture(rhs)? {
                            Bool(b) => Ok(Bool(b)),
                            rhs_value => binary_op_error(lhs_value, rhs_value),
                        }
                    } else {
                        Ok(Bool(false))
                    }
                } else {
                    runtime_error!(
                        node,
                        "'and' only works with Bools, found '{}'",
                        type_as_string(&lhs_value)
                    )
                }
            }
            AstOp::Or => {
                return if let Bool(a) = lhs_value {
                    if !a {
                        match self.evaluate_and_capture(rhs)? {
                            Bool(b) => Ok(Bool(b)),
                            rhs_value => binary_op_error(lhs_value, rhs_value),
                        }
                    } else {
                        Ok(Bool(true))
                    }
                } else {
                    runtime_error!(
                        node,
                        "'or' only works with Bools, found '{}'",
                        type_as_string(&lhs_value)
                    )
                }
            }
            _ => {}
        }

        let rhs_value = self.evaluate_and_capture(rhs)?;

        runtime_trace!(self, "{:?} - lhs: {} rhs: {}", op, &lhs_value, &rhs_value);

        match op {
            AstOp::Equal => Ok((lhs_value == rhs_value).into()),
            AstOp::NotEqual => Ok((lhs_value != rhs_value).into()),
            _ => match (&lhs_value, &rhs_value) {
                (Number(a), Number(b)) => match op {
                    AstOp::Add => Ok(Number(a + b)),
                    AstOp::Subtract => Ok(Number(a - b)),
                    AstOp::Multiply => Ok(Number(a * b)),
                    AstOp::Divide => Ok(Number(a / b)),
                    AstOp::Modulo => Ok(Number(a % b)),
                    AstOp::Less => Ok(Bool(a < b)),
                    AstOp::LessOrEqual => Ok(Bool(a <= b)),
                    AstOp::Greater => Ok(Bool(a > b)),
                    AstOp::GreaterOrEqual => Ok(Bool(a >= b)),
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Vec4(a), Vec4(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Number(a), Vec4(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Vec4(a), Number(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Bool(_), Bool(_)) => match op {
                    AstOp::And | AstOp::Or => unreachable!(), // handled earlier
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (List(a), List(b)) => match op {
                    AstOp::Add => {
                        let mut result = ValueVec::clone(&a.data());
                        result.extend(ValueVec::clone(&b.data()).into_iter());
                        Ok(List(ValueList::with_data(result)))
                    }
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Map(a), Map(b)) => match op {
                    AstOp::Add => {
                        let mut result = a.data().clone();
                        result.extend(&b.data());
                        Ok(Map(ValueMap::with_data(result)))
                    }
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                (Str(a), Str(b)) => match op {
                    AstOp::Add => {
                        let result = String::clone(a) + b.as_ref();
                        Ok(Str(Arc::new(result)))
                    }
                    _ => binary_op_error(lhs_value, rhs_value),
                },
                _ => binary_op_error(lhs_value, rhs_value),
            },
        }
    }

    fn do_if_statement(&mut self, if_statement: &AstIf, node: &AstNode) -> RuntimeResult<'a> {
        use Value::{Bool, Empty};

        let AstIf {
            condition,
            then_node,
            else_if_condition,
            else_if_node,
            else_node,
        } = if_statement;

        let maybe_bool = self.evaluate(condition)?;
        if let Bool(condition_value) = maybe_bool {
            if condition_value {
                return self.evaluate(then_node);
            }

            if else_if_condition.is_some() {
                let maybe_bool = self.evaluate(&else_if_condition.as_ref().unwrap())?;
                if let Bool(condition_value) = maybe_bool {
                    if condition_value {
                        return self.evaluate(else_if_node.as_ref().unwrap());
                    }
                } else {
                    return runtime_error!(
                        node,
                        "Expected bool in else if statement, found {}",
                        type_as_string(&maybe_bool)
                    );
                }
            }

            if else_node.is_some() {
                return self.evaluate(else_node.as_ref().unwrap());
            }

            Ok(Empty)
        } else {
            return runtime_error!(
                node,
                "Expected bool in if statement, found {}",
                type_as_string(&maybe_bool)
            );
        }
    }

    fn make_vec4(&mut self, expressions: &[AstNode], node: &AstNode) -> RuntimeResult<'a> {
        use Value::{List, Number, Vec4};

        let v = match expressions {
            [expression] => match &self.evaluate_and_capture(expression)? {
                Number(n) => {
                    let n = *n as f32;
                    vec4::Vec4(n, n, n, n)
                }
                Vec4(v) => *v,
                List(list) => {
                    let mut v = vec4::Vec4::default();
                    for (i, value) in list.data().iter().take(4).enumerate() {
                        match value {
                            Number(n) => v[i] = *n as f32,
                            unexpected => {
                                return runtime_error!(
                                    node,
                                    "vec4 only accepts Numbers as arguments, - found {}",
                                    unexpected
                                )
                            }
                        }
                    }
                    v
                }
                unexpected => {
                    return runtime_error!(
                        node,
                        "vec4 only accepts a Number, Vec4, or List as first argument - found {}",
                        unexpected
                    );
                }
            },
            _ => {
                let mut v = vec4::Vec4::default();
                for (i, expression) in expressions.iter().take(4).enumerate() {
                    match &self.evaluate(expression)? {
                        Number(n) => v[i] = *n as f32,
                        unexpected => {
                            return runtime_error!(
                                node,
                                "vec4 only accepts Numbers as arguments, \
                                    or Vec4 or List as first argument - found {}",
                                unexpected
                            );
                        }
                    }
                }
                v
            }
        };
        Ok(Vec4(v))
    }

    fn get_constant_string(&self, constant_index: &u32) -> &str {
        self.constants.get_string(*constant_index as usize)
    }

    fn arc_string_from_constant(&mut self, constant_index: &u32) -> Arc<String> {
        match self.string_constants.get(constant_index) {
            Some(s) => s.clone(),
            None => {
                let s = Arc::new(
                    self.constants
                        .get_string(*constant_index as usize)
                        .to_string(),
                );
                self.string_constants.insert(*constant_index, s.clone());
                s
            }
        }
    }

    fn id_from_constant(&mut self, constant_index: &u32) -> Id {
        Id::new(self.arc_string_from_constant(constant_index))
    }

    pub fn str_to_id_index(&self, s: &str) -> Option<u32> {
        for (k, v) in self.string_constants.iter() {
            if v.as_ref() == s {
                return Some(*k);
            }
        }
        None
    }

    fn lookup_slice_to_string(&self, lookup_slice: &LookupSlice) -> String {
        let mut result = String::new();

        let mut first = true;
        for node in lookup_slice.0.iter() {
            match &node {
                LookupNode::Id(id_index) => {
                    if !first {
                        result += ".";
                    }
                    result += self.get_constant_string(id_index);
                }
                LookupNode::Index(index) => {
                    let expression = match index.0.node {
                        Node::Number(n_index) => {
                            self.constants.get_f64(n_index as usize).to_string()
                        }
                        _ => "...".to_string(),
                    };
                    result += &format!("[{}]", expression);
                }
                LookupNode::Call(_) => {
                    result += "()";
                }
            }
            first = false;
        }

        result
    }

    fn lookup_slice_or_id_to_string(&self, lookup_or_id: &LookupSliceOrId) -> String {
        match lookup_or_id {
            LookupSliceOrId::Id(id_index) => self.get_constant_string(id_index).to_string(),
            LookupSliceOrId::LookupSlice(lookup_slice) => self.lookup_slice_to_string(lookup_slice),
        }
    }
}

fn is_single_value_node(node: &Node) -> bool {
    use Node::*;
    match node {
        For(_) | While(_) | Range { .. } | RangeFrom { .. } | RangeTo { .. } | RangeFull => false,
        _ => true,
    }
}
