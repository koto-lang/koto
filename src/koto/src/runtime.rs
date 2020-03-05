use crate::{
    call_stack::CallStack,
    runtime_error,
    value::{deref_value, make_reference, type_as_string, values_have_matching_type, Value},
    value_iterator::{MultiRangeValueIterator, ValueIterator},
    Error, Id, LookupSlice, RuntimeResult, ValueList, ValueMap,
};
use koto_parser::{
    AssignTarget, AstFor, AstIf, AstNode, AstOp, LookupNode, LookupOrId, Node, Scope,
};
use std::{cell::RefCell, path::Path, rc::Rc};

enum ValueOrValues<'a> {
    Value(Value<'a>),
    Values(Vec<Value<'a>>),
}

#[derive(Default)]
pub struct Environment {
    pub script_path: Option<String>,
    pub args: Vec<String>,
}

#[derive(Default)]
pub struct Runtime<'a> {
    environment: Environment,
    global: ValueMap<'a>,
    call_stack: CallStack<'a>,
}

#[cfg(feature = "trace")]
macro_rules! runtime_trace  {
    ($self:expr, $message:expr) => {
        println!("{}{}", $self.runtime_indent(), $message);
    };
    ($self:expr, $message:expr, $($vals:expr),+) => {
        println!("{}{}", $self.runtime_indent(), format!($message, $($vals),+));
    };
}

#[cfg(not(feature = "trace"))]
macro_rules! runtime_trace {
    ($self:expr, $message:expr) => {};
    ($self:expr, $message:expr, $($vals:expr),+) => {};
}

impl<'a> Runtime<'a> {
    pub fn new() -> Self {
        let mut result = Self {
            environment: Default::default(),
            global: ValueMap::with_capacity(32),
            call_stack: CallStack::new(),
        };
        crate::builtins::register(&mut result);
        result
    }

    pub fn environment_mut(&mut self) -> &mut Environment {
        &mut self.environment
    }

    pub fn setup_environment(&mut self) {
        use Value::{Empty, Str};

        let (script_dir, script_path) = match &self.environment.script_path {
            Some(path) => (
                Path::new(&path)
                    .parent()
                    .map(|p| {
                        Str(Rc::new(
                            p.to_str().expect("invalid script path").to_string(),
                        ))
                    })
                    .or(Some(Empty))
                    .unwrap(),
                Str(Rc::new(path.to_string())),
            ),
            None => (Empty, Empty),
        };
        let mut args = vec![script_path];
        for arg in self.environment.args.iter() {
            args.push(Str(Rc::new(arg.to_string())));
        }

        let mut env = ValueMap::new();

        env.add_value("script_dir", script_dir);
        env.add_list("args", ValueList::with_data(args));

        self.global.add_map("env", env);
    }

    pub fn global_mut(&mut self) -> &mut ValueMap<'a> {
        &mut self.global
    }

    #[allow(dead_code)]
    fn runtime_indent(&self) -> String {
        // TODO maintain indent count when trace is enabled
        "  ".to_string()
    }

    /// Run a script and capture the final value
    pub fn run(&mut self, ast: &[AstNode]) -> RuntimeResult<'a> {
        runtime_trace!(self, "run");

        self.evaluate_block(ast)
    }

    /// Evaluate a series of expressions and return the final result
    fn evaluate_block(&mut self, block: &[AstNode]) -> RuntimeResult<'a> {
        runtime_trace!(self, "evaluate_block - {}", block.len());

        for (i, expression) in block.iter().enumerate() {
            if i < block.len() - 1 {
                self.evaluate_and_expand(expression, false)?;
            } else {
                return self.evaluate_and_capture(expression);
            }
        }

        unreachable!();
    }

    /// Evaluate a series of expressions and capture their results in a list
    fn evaluate_expressions(
        &mut self,
        expressions: &[AstNode],
    ) -> Result<ValueOrValues<'a>, Error> {
        runtime_trace!(self, "evaluate_expressions - {}", expressions.len());

        if expressions.len() == 1 {
            Ok(ValueOrValues::Value(
                self.evaluate_and_capture(&expressions[0])?,
            ))
        } else {
            let mut results = Vec::new();

            for expression in expressions.iter() {
                if koto_parser::is_single_value_node(&expression.node) {
                    results.push(self.evaluate(expression)?);
                } else {
                    results.push(self.evaluate_and_capture(expression)?);
                }
            }

            Ok(ValueOrValues::Values(results))
        }
    }

    /// Evaluate an expression and capture multiple return values in a List
    fn evaluate_and_capture(&mut self, expression: &AstNode) -> RuntimeResult<'a> {
        use Value::*;

        runtime_trace!(self, "evaluate_and_capture - {}", expression.node);

        if koto_parser::is_single_value_node(&expression.node) {
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
                        .collect::<Result<Vec<_>, Error>>()?;
                    Ok(List(Rc::new(ValueList::with_data(list))))
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
            For(_) | Range { .. } => true,
            _ => false,
        };

        let result = if expand_value {
            match value {
                For(for_loop) => self.run_for_loop(&for_loop, expression, capture)?,
                Range { min, max } => {
                    if capture {
                        let expanded = (min..max).map(|n| Number(n as f64)).collect::<Vec<_>>();
                        List(Rc::new(ValueList::with_data(expanded)))
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

    fn evaluate(&mut self, node: &AstNode) -> RuntimeResult<'a> {
        runtime_trace!(self, "evaluate - {}", node.node);

        use Value::*;

        let result = match &node.node {
            Node::Bool(b) => Bool(*b),
            Node::Number(n) => Number(*n),
            Node::Vec4(v) => Vec4(*v),
            Node::Str(s) => Str(s.clone()),
            Node::List(elements) => match self.evaluate_expressions(elements)? {
                ValueOrValues::Value(value) => match value {
                    List(_) => value,
                    _ => List(Rc::new(ValueList::with_data(vec![value]))),
                },
                ValueOrValues::Values(values) => Value::List(Rc::new(ValueList::with_data(values))),
            },
            Node::Range {
                min,
                max,
                inclusive,
            } => self.make_range(min, max, *inclusive, node)?,
            Node::Map(entries) => {
                let mut map = ValueMap::with_capacity(entries.len());
                for (id, node) in entries.iter() {
                    let value = self.evaluate_and_capture(node)?;
                    map.insert(id.clone(), value);
                }
                Map(Rc::new(map))
            }
            Node::Lookup(lookup) => {
                let (value, _scope) = self.lookup_value_or_error(&lookup.as_slice(), node)?;
                value
            }
            Node::Id(id) => {
                let (value, _scope) = self.get_value_or_error(&id, node)?;
                value
            }
            Node::Ref(lookup_or_id) => match lookup_or_id {
                LookupOrId::Id(id) => self.make_reference_from_id(&id, node)?,
                LookupOrId::Lookup(lookup) => {
                    self.make_reference_from_lookup(&lookup.as_slice(), node)?
                }
            },
            Node::Block(block) => self.evaluate_block(&block)?,
            Node::Expressions(expressions) => match self.evaluate_expressions(expressions)? {
                ValueOrValues::Value(value) => value,
                ValueOrValues::Values(values) => Value::List(Rc::new(ValueList::with_data(values))),
            },
            Node::RefExpression(expression) => {
                let value = self.evaluate_and_capture(expression)?;
                match value {
                    Ref(_) => value,
                    _ => Ref(Rc::new(RefCell::new(value))),
                }
            }
            Node::Negate(expression) => {
                let value = self.evaluate_and_capture(expression)?;
                match value {
                    Bool(b) => Bool(!b),
                    unexpected => {
                        return runtime_error!(
                            node,
                            "Expected Bool for not operator, found {}",
                            unexpected
                        );
                    }
                }
            }
            Node::Function(f) => Function(f.clone()),
            Node::Call { function, args } => self.call_function(function, args, node)?,
            Node::Assign { target, expression } => self.assign_value(target, expression, node)?,
            Node::MultiAssign {
                targets,
                expressions,
            } => self.assign_values(targets, expressions, node)?,
            Node::Op { op, lhs, rhs } => self.binary_op(op, lhs, rhs, node)?,
            Node::If(if_statement) => self.do_if_statement(if_statement, node)?,
            Node::For(f) => For(f.clone()),
        };

        Ok(result)
    }

    fn set_value(&mut self, id: &Id, value: Value<'a>, scope: Scope) {
        use Value::Ref;

        runtime_trace!(self, "set_value - {}: {} - {:?}", id, value, scope);

        if self.call_stack.frame() == 0 || scope == Scope::Global {
            match self.global.0.get_mut(id.as_ref()) {
                Some(exists) => match (&exists, &value) {
                    (Ref(ref_a), Ref(ref_b)) => {
                        if ref_a != ref_b {
                            *exists = value;
                        }
                    }
                    (Ref(ref_a), _) if values_have_matching_type(&exists, &value) => {
                        *ref_a.borrow_mut() = value;
                    }
                    _ => {
                        *exists = value;
                    }
                },
                None => {
                    self.global.0.insert(id.clone(), value);
                }
            }
        } else {
            match self.call_stack.get_mut(id.as_ref()) {
                Some(exists) => match (&exists, &value) {
                    (Ref(ref_a), Ref(ref_b)) => {
                        if ref_a != ref_b {
                            *exists = value;
                        }
                    }
                    (Ref(ref_a), _) if values_have_matching_type(&exists, &value) => {
                        *ref_a.borrow_mut() = value;
                    }
                    _ => {
                        *exists = value;
                    }
                },
                None => {
                    self.call_stack.extend(id.clone(), value);
                }
            }
        }
    }

    fn visit_value_mut<'b: 'a>(
        &mut self,
        lookup_id: &LookupSlice,
        node: &AstNode,
        mut visitor: impl FnMut(&LookupSlice, &AstNode, &mut Value<'a>) -> Result<(), Error>
            + Clone
            + 'b,
    ) -> Result<(), Error> {
        runtime_trace!(self, "visit_value_mut - {}", lookup_id);

        macro_rules! do_visit {
            ($value:expr) => {{
                match $value {
                    Some(value) => {
                        if lookup_id.0.len() == 1 {
                            return visitor(lookup_id, node, value);
                        } else {
                            match value {
                                Value::Map(map) => {
                                    let (found, error) = Rc::make_mut(map).visit_mut(
                                        lookup_id,
                                        1,
                                        node,
                                        visitor.clone(),
                                    );
                                    (found, Some(error))
                                }
                                // Value::List(list) => {}
                                _ => (false, None),
                            }
                        }
                    }
                    _ => (false, None),
                }
            }};
        }

        let first_id = match &lookup_id.0.first().unwrap() {
            LookupNode::Id(id) => id,
            LookupNode::Index(index) => &index
                .id
                .as_ref()
                .expect("Expected non-nested list id for first lookup"),
        };

        if self.call_stack.frame() > 0 {
            let value = self.call_stack.get_mut(&first_id);
            match do_visit!(value) {
                (false, _) => {}
                (true, Some(result)) => {
                    return result;
                }
                _ => unreachable!(),
            }
        }

        let global_value = self.global.0.get_mut(first_id);
        match do_visit!(global_value) {
            (false, None) => runtime_error!(node, "'{}' not found", lookup_id),
            (false, Some(result)) => result,
            (true, Some(result)) => result,
            _ => unreachable!(),
        }
    }

    fn get_value(&self, id: &Id) -> Option<(Value<'a>, Scope)> {
        if self.call_stack.frame() > 0 {
            if let Some(value) = self.call_stack.get(id) {
                return Some((value.clone(), Scope::Local));
            }
        }

        match self.global.0.get(id) {
            Some(value) => Some((value.clone(), Scope::Global)),
            None => None,
        }
    }

    fn get_value_or_error(&self, id: &Id, node: &AstNode) -> Result<(Value<'a>, Scope), Error> {
        match self.get_value(id) {
            Some(v) => Ok(v),
            None => runtime_error!(node, "'{}' not found", id),
        }
    }

    fn lookup_value(
        &mut self,
        lookup: &LookupSlice,
        node: &AstNode,
    ) -> Result<Option<(Value<'a>, Scope)>, Error> {
        macro_rules! do_lookup {
            ($value:expr) => {{
                match $value {
                    Some(value) => {
                        if lookup.0.len() == 1 {
                            match &lookup.0[0] {
                                LookupNode::Id(_) => Some(value),
                                LookupNode::Index(index) => match deref_value(&value) {
                                    Value::List(data) => Some(self.list_index(
                                        &data,
                                        &lookup,
                                        &index.expression,
                                        node,
                                    )?),
                                    unexpected => {
                                        return runtime_error!(
                                            node,
                                            "Expected list for '{}', found {}",
                                            lookup,
                                            type_as_string(&unexpected)
                                        );
                                    }
                                },
                            }
                        } else {
                            let mut result = if let LookupNode::Index(index) = &lookup.0[0] {
                                // at this point we have the list, but we need the deindexed value
                                match deref_value(&value) {
                                    Value::List(data) => Some(self.list_index(
                                        &data,
                                        &lookup,
                                        &index.expression,
                                        node,
                                    )?),
                                    unexpected => {
                                        return runtime_error!(
                                            node,
                                            "Expected list for '{}', found {}",
                                            lookup,
                                            type_as_string(&unexpected)
                                        );
                                    }
                                }
                            } else {
                                Some(value)
                            };

                            for (i, lookup_node) in lookup.0[1..].iter().enumerate() {
                                match result.clone().map(|v| deref_value(&v)) {
                                    Some(Value::Map(data)) => match lookup_node {
                                        LookupNode::Id(id) => {
                                            result = data.0.get(id).map(|v| v.clone());
                                        }
                                        LookupNode::Index(index) => {
                                            match data.0.get(
                                                &index
                                                    .id
                                                    .as_ref()
                                                    .expect("Expected a list id for map lookup")
                                                    .clone(),
                                            ) {
                                                Some(Value::List(data)) => {
                                                    result = Some(self.list_index(
                                                        &data,
                                                        &lookup.slice(0, i + 1),
                                                        &index.expression,
                                                        node,
                                                    )?);
                                                }
                                                Some(unexpected) => {
                                                    return runtime_error!(
                                                        node,
                                                        "Expected list for '{}', found {}",
                                                        lookup,
                                                        type_as_string(&unexpected)
                                                    );
                                                }
                                                None => break,
                                            }
                                        }
                                    },
                                    Some(Value::List(data)) => match lookup_node {
                                        LookupNode::Id(id) => {
                                            return runtime_error!(
                                                node,
                                                "Found a list instead of a Map for {}",
                                                id
                                            );
                                        }
                                        LookupNode::Index(index) => {
                                            result = Some(self.list_index(
                                                &data,
                                                &lookup.slice(0, i + 1),
                                                &index.expression,
                                                node,
                                            )?);
                                        }
                                    },
                                    _ => break,
                                }
                            }
                            result
                        }
                    }
                    None => None,
                }
            }};
        }

        let first_id = match &lookup.0[0] {
            LookupNode::Id(id) => id,
            LookupNode::Index(index) => &index
                .id
                .as_ref()
                .expect("Expected non-nested list id for first lookup"),
        };
        if self.call_stack.frame() > 0 {
            let value = self.call_stack.get(first_id).cloned();
            if let Some(value) = do_lookup!(value) {
                return Ok(Some((value, Scope::Local)));
            }
        }

        let global_value = self.global.0.get(first_id).cloned();
        match do_lookup!(global_value) {
            Some(value) => Ok(Some((value, Scope::Global))),
            None => Ok(None),
        }
    }

    fn lookup_value_or_error(
        &mut self,
        id: &LookupSlice,
        node: &AstNode,
    ) -> Result<(Value<'a>, Scope), Error> {
        match self.lookup_value(id, node)? {
            Some(v) => Ok(v),
            None => runtime_error!(node, "'{}' not found", id),
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

        let mut captured = Vec::new();

        if f.ranges.len() == 1 {
            let range = self.evaluate(f.ranges.first().unwrap())?;

            let value_iter = match deref_value(&range) {
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
                    self.set_value(first_arg, value.clone(), Scope::Local);
                } else {
                    let mut arg_iter = f.args.iter().peekable();
                    match value {
                        List(a) => {
                            for list_value in a.data().iter() {
                                match arg_iter.next() {
                                    Some(arg) => {
                                        self.set_value(arg, list_value.clone(), Scope::Local)
                                    }
                                    None => break,
                                }
                            }
                        }
                        _ => self.set_value(
                            arg_iter
                                .next()
                                .expect("For loops have at least one argument"),
                            value.clone(),
                            Scope::Local,
                        ),
                    }
                    for remaining_arg in arg_iter {
                        self.set_value(remaining_arg, Value::Empty, Scope::Local);
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

                let result = self.evaluate_and_capture(&f.body)?;
                if capture {
                    captured.push(result);
                }
            }
        } else {
            let mut multi_range_iterator = MultiRangeValueIterator::with_capacity(f.ranges.len());
            for range in f.ranges.iter() {
                let range = self.evaluate(range)?;

                match deref_value(&range) {
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

            let mut values = Vec::new();

            while multi_range_iterator.get_next_values(&mut values) {
                if single_arg {
                    if values.len() == 1 {
                        self.set_value(first_arg, values[0].clone(), Scope::Local);
                    } else {
                        self.set_value(
                            first_arg,
                            Value::List(Rc::new(ValueList::with_data(values.clone()))),
                            Scope::Local,
                        );
                    }
                } else {
                    let mut arg_iter = f.args.iter().peekable();
                    for value in values.iter() {
                        match arg_iter.next() {
                            Some(arg) => {
                                self.set_value(arg, value.clone(), Scope::Local);
                            }
                            None => break,
                        }
                    }
                    for remaining_arg in arg_iter {
                        self.set_value(remaining_arg, Value::Empty, Scope::Local);
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
                let result = self.evaluate_and_capture(&f.body)?;
                if capture {
                    captured.push(result);
                }
            }
        }

        Ok(if captured.is_empty() {
            Empty
        } else {
            List(Rc::new(ValueList::with_data(captured)))
        })
    }

    fn set_map_value(
        &mut self,
        lookup: &LookupSlice,
        value: Value<'a>,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        runtime_trace!(self, "set_map_value - {}: {}", lookup, &value);

        let value_id = match lookup.0.last().unwrap().clone() {
            LookupNode::Id(id) => id,
            LookupNode::Index(_) => unreachable!(),
        };

        self.visit_value_mut(
            &lookup.parent_slice(),
            node,
            move |map_lookup, node, maybe_map| {
                use Value::{Map, Ref};

                match maybe_map {
                    Map(map) => {
                        Rc::make_mut(map).add_value(&value_id, value.clone());
                        Ok(())
                    }
                    Ref(r) => match &mut *r.borrow_mut() {
                        Map(map) => {
                            Rc::make_mut(map).add_value(&value_id, value.clone());
                            Ok(())
                        }
                        unexpected => runtime_error!(
                            node,
                            "Expected Map for '{}', found {}",
                            map_lookup,
                            type_as_string(&unexpected)
                        ),
                    },
                    _ => runtime_error!(
                        node,
                        "Expected Map for '{}', found {}",
                        map_lookup,
                        type_as_string(&maybe_map)
                    ),
                }
            },
        )?;

        Ok(Value::Empty)
    }

    fn set_list_value(
        &mut self,
        id: &LookupSlice,
        expression: &AstNode,
        value: Value<'a>,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::*;

        runtime_trace!(self, "set_list_value - {}: {}", id, &value);

        let index = self.evaluate(expression)?;

        self.visit_value_mut(id, node, move |id, node, maybe_list| {
            let assign_to_index = |list: &mut ValueList<'a>| match index {
                Number(i) => {
                    let i = i as usize;
                    if i < list.data().len() {
                        list.data_mut()[i] = value.clone();
                        Ok(Empty)
                    } else {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} but the index is {}",
                            id,
                            list.data().len(),
                            i
                        )
                    }
                }
                Range { min, max } => {
                    let umin = min as usize;
                    let umax = max as usize;
                    if min < 0 || max < 0 {
                        runtime_error!(
                            node,
                            "Indexing with negative indices isn't supported, min: {}, max: {}",
                            min,
                            max
                        )
                    } else if umin >= list.data().len() || umax > list.data().len() {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                            id,
                            list.data().len(),
                            min,
                            max
                        )
                    } else {
                        for element in &mut list.data_mut()[umin..umax] {
                            *element = value.clone();
                        }
                        Ok(Empty)
                    }
                }
                _ => runtime_error!(
                    node,
                    "Indexing is only supported with number values or ranges, found {})",
                    type_as_string(&index)
                ),
            };

            match maybe_list {
                List(data) => {
                    assign_to_index(&mut Rc::make_mut(data))?;
                }
                Ref(r) => match &mut *r.borrow_mut() {
                    List(data) => {
                        assign_to_index(&mut Rc::make_mut(data))?;
                    }
                    unexpected => {
                        return runtime_error!(
                            node,
                            "Indexing is only supported for Lists, found {}",
                            type_as_string(&unexpected)
                        );
                    }
                },
                _ => {
                    return runtime_error!(
                        node,
                        "Indexing is only supported for Lists, found {}",
                        type_as_string(&maybe_list)
                    )
                }
            };
            Ok(())
        })?;

        Ok(Empty)
    }

    fn list_index(
        &mut self,
        list: &ValueList<'a>,
        list_id: &LookupSlice,
        expression: &AstNode,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::*;

        let index = self.evaluate(expression)?;

        let result = match index {
            Number(i) => {
                let i = i as usize;
                if i < list.data().len() {
                    list.data()[i].clone()
                } else {
                    return runtime_error!(
                        node,
                        "Index out of bounds: '{}' has a length of {} but the index is {}",
                        list_id,
                        list.data().len(),
                        i
                    );
                }
            }
            Range { min, max } => {
                let umin = min as usize;
                let umax = max as usize;
                if min < 0 || max < 0 {
                    return runtime_error!(
                        node,
                        "Indexing with negative indices isn't supported, min: {}, max: {}",
                        min,
                        max
                    );
                } else if umin >= list.data().len() || umax >= list.data().len() {
                    return runtime_error!(
                        node,
                        "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                        list_id,
                        list.data().len(),
                        min,
                        max
                    );
                } else {
                    // TODO Avoid allocating new vec, introduce 'slice' value type
                    List(Rc::new(ValueList::with_data(
                        list.data()[umin..umax].to_vec(),
                    )))
                }
            }
            _ => {
                return runtime_error!(
                    node,
                    "Indexing is only supported with number values or ranges, found {})",
                    type_as_string(&index)
                )
            }
        };

        Ok(result)
    }

    fn call_function(
        &mut self,
        lookup_or_id: &LookupOrId,
        args: &[AstNode],
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::*;

        runtime_trace!(self, "call_function - {}", lookup_or_id);

        let maybe_function = match lookup_or_id {
            LookupOrId::Id(id) => self.get_value(&id),
            LookupOrId::Lookup(lookup) => self.lookup_value(&lookup.as_slice(), node)?,
        };

        let maybe_function = match maybe_function {
            Some((ExternalFunction(f), _)) => {
                let args_for_builtin = self.evaluate_expressions(args)?; // TODO optimize
                let mut closure = f.0.borrow_mut();
                let builtin_result = match args_for_builtin {
                    ValueOrValues::Value(value) => (&mut *closure)(&[value]),
                    ValueOrValues::Values(values) => (&mut *closure)(&values),
                };
                return match builtin_result {
                    Ok(value) => Ok(value),
                    Err(e) => return runtime_error!(node, e),
                };
            }
            Some((Function(f), _)) => Some(f),
            Some((unexpected, _)) => {
                return runtime_error!(
                    node,
                    "Expected '{}' to be a Function, found {}",
                    lookup_or_id,
                    type_as_string(&unexpected)
                )
            }
            None => None,
        };

        if let Some(f) = maybe_function {
            let mut implicit_self = false;

            match lookup_or_id {
                LookupOrId::Id(id) => {
                    // allow standalone functions to be able to call themselves
                    self.call_stack.push(id.clone(), Function(f.clone()));
                }
                LookupOrId::Lookup(lookup) => {
                    // implicit self for map functions
                    match f.args.first() {
                        Some(self_arg) if self_arg.as_ref() == "self" => {
                            let map_id = lookup.parent_slice();
                            self.make_reference_from_lookup(&map_id, node)?;
                            let (map, _scope) = self.lookup_value(&map_id, node)?.unwrap();
                            self.call_stack.push(self_arg.clone(), map);
                            implicit_self = true;
                        }
                        _ => {}
                    }
                }
            }

            let arg_count = f.args.len();
            let expected_args = if implicit_self {
                arg_count - 1
            } else {
                arg_count
            };

            if args.len() != expected_args {
                return runtime_error!(
                    node,
                    "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                    lookup_or_id,
                    expected_args,
                    args.len(),
                    f.args
                );
            }

            for (name, arg) in f
                .args
                .iter()
                .skip(if implicit_self { 1 } else { 0 })
                .zip(args.iter())
            {
                let arg_value = match self.evaluate_and_capture(arg) {
                    Ok(value) => value,
                    e @ Err(_) => {
                        self.call_stack.cancel();
                        return e;
                    }
                };

                self.call_stack.push(name.clone(), arg_value);
            }

            self.call_stack.commit();
            let result = self.evaluate_block(&f.body);
            self.call_stack.pop_frame();
            return result;
        }

        runtime_error!(node, "Function '{}' not found", lookup_or_id)
    }

    fn make_reference_from_id(&mut self, id: &Id, node: &AstNode) -> RuntimeResult<'a> {
        let (value, scope) = self.get_value_or_error(&id, node)?;
        let (value_ref, made_ref) = make_reference(value);
        if made_ref {
            self.set_value(id, value_ref.clone(), scope);
        }
        Ok(value_ref)
    }

    fn make_reference_from_lookup(
        &mut self,
        lookup: &LookupSlice,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        match lookup.0.last().unwrap() {
            LookupNode::Id(id) => {
                if lookup.0.len() == 1 {
                    self.make_reference_from_id(id, node)
                } else {
                    let (value, _scope) = self.lookup_value_or_error(lookup, node)?;

                    let (value_ref, made_ref) = make_reference(value);
                    if made_ref {
                        self.set_map_value(&lookup, value_ref.clone(), node)?;
                    }

                    Ok(value_ref)
                }
            }
            LookupNode::Index(index) => {
                let (value, _scope) = self.lookup_value_or_error(lookup, node)?;

                let (value_ref, made_ref) = make_reference(value);
                if made_ref {
                    self.set_list_value(&lookup, &index.expression, value_ref.clone(), node)?;
                }

                Ok(value_ref)
            }
        }
    }

    fn assign_value(
        &mut self,
        target: &AssignTarget,
        expression: &AstNode,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        let value = self.evaluate_and_capture(expression)?;

        match target {
            AssignTarget::Id { id, scope } => {
                self.set_value(id, value.clone(), *scope);
            }
            AssignTarget::Lookup(lookup) => match lookup.value_node() {
                LookupNode::Id(_) => {
                    self.set_map_value(&lookup.as_slice(), value.clone(), node)?;
                }
                LookupNode::Index(index) => {
                    self.set_list_value(
                        &lookup.as_slice(),
                        &index.expression,
                        value.clone(),
                        node,
                    )?;
                }
            },
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
                    AssignTarget::Id { id, scope } => {
                        self.set_value(&id, $value, *scope);
                    }
                    AssignTarget::Lookup(lookup) => match lookup.value_node() {
                        LookupNode::Id(_) => {
                            self.set_map_value(&lookup.as_slice(), $value, node)?;
                        }
                        LookupNode::Index(index) => {
                            self.set_list_value(
                                &lookup.as_slice(),
                                &index.expression,
                                $value,
                                node,
                            )?;
                        }
                    },
                }
            };
        };

        if expressions.len() == 1 {
            let value = self.evaluate_and_capture(&expressions[0])?;

            match &value {
                List(l) => {
                    let mut result_iter = l.data().iter();
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
            let mut results = Vec::new();

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
            Ok(List(Rc::new(ValueList::with_data(results))))
        }
    }

    fn make_range(
        &mut self,
        min: &AstNode,
        max: &AstNode,
        inclusive: bool,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::{Number, Range};

        let min = self.evaluate(min)?;
        let max = self.evaluate(max)?;

        match (min, max) {
            (Number(min), Number(max)) => {
                let min = min as isize;
                let max = max as isize;
                let max = if inclusive { max + 1 } else { max };
                if min <= max {
                    Ok(Range { min, max })
                } else {
                    return runtime_error!(
                        node,
                        "Invalid range, min should be less than or equal to max - min: {}, max: {}",
                        min,
                        max
                    );
                }
            }
            unexpected => {
                return runtime_error!(
                    node,
                    "Expected numbers for range bounds, found min: {}, max: {}",
                    type_as_string(&unexpected.0),
                    type_as_string(&unexpected.1)
                )
            }
        }
    }

    fn binary_op(
        &mut self,
        op: &AstOp,
        lhs: &AstNode,
        rhs: &AstNode,
        node: &AstNode,
    ) -> RuntimeResult<'a> {
        use Value::*;

        let a = self.evaluate_and_capture(lhs)?;
        let b = self.evaluate_and_capture(rhs)?;

        runtime_trace!(self, "{:?} - a: {} b: {}", op, &a, &b);

        macro_rules! binary_op_error {
            ($op:ident, $a:ident, $b:ident) => {
                runtime_error!(
                    node,
                    "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
                    op,
                    a,
                    b
                )
            };
        };

        match op {
            AstOp::Equal => Ok((a == b).into()),
            AstOp::NotEqual => Ok((a != b).into()),
            _ => match (&a, &b) {
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
                    _ => binary_op_error!(op, a, b),
                },
                (Vec4(a), Vec4(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error!(op, a, b),
                },
                (Number(a), Vec4(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error!(op, a, b),
                },
                (Vec4(a), Number(b)) => match op {
                    AstOp::Add => Ok(Vec4(*a + *b)),
                    AstOp::Subtract => Ok(Vec4(*a - *b)),
                    AstOp::Multiply => Ok(Vec4(*a * *b)),
                    AstOp::Divide => Ok(Vec4(*a / *b)),
                    AstOp::Modulo => Ok(Vec4(*a % *b)),
                    _ => binary_op_error!(op, a, b),
                },
                (Bool(a), Bool(b)) => match op {
                    AstOp::And => Ok(Bool(*a && *b)),
                    AstOp::Or => Ok(Bool(*a || *b)),
                    _ => binary_op_error!(op, a, b),
                },
                (List(a), List(b)) => match op {
                    AstOp::Add => {
                        let mut result = Vec::clone(a.data());
                        result.extend(Vec::clone(b.data()).into_iter());
                        Ok(List(Rc::new(ValueList::with_data(result))))
                    }
                    _ => binary_op_error!(op, a, b),
                },
                (Map(a), Map(b)) => match op {
                    AstOp::Add => {
                        let mut result = a.0.clone();
                        result.extend(b.0.clone().into_iter());
                        Ok(Map(Rc::new(ValueMap(result))))
                    }
                    _ => binary_op_error!(op, a, b),
                },
                (Str(a), Str(b)) => match op {
                    AstOp::Add => {
                        let result = String::clone(a) + b.as_ref();
                        Ok(Str(Rc::new(result)))
                    }
                    _ => binary_op_error!(op, a, b),
                },
                _ => binary_op_error!(op, a, b),
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
}
