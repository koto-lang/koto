use crate::{
    call_stack::CallStack,
    runtime_error,
    value::{deref_value, type_as_string, values_have_matching_type, Value},
    value_iterator::{MultiRangeValueIterator, ValueIterator},
    value_map::ValueMap,
    value_stack::ValueStack,
    Error, Id, LookupId, LookupIdSlice, RuntimeResult,
};
use koto_parser::{AssignTarget, AstIndex, AstNode, AstOp, Node, Scope};
use std::{cell::RefCell, rc::Rc};

pub struct Runtime<'a> {
    global: ValueMap<'a>,
    call_stack: CallStack<'a>,
    value_stack: ValueStack<'a>,
    multi_range_iterator: MultiRangeValueIterator<'a>,
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
            global: ValueMap::with_capacity(32),
            call_stack: CallStack::new(),
            value_stack: ValueStack::new(),
            multi_range_iterator: MultiRangeValueIterator::with_capacity(4),
        };
        crate::builtins::register(&mut result);
        result
    }

    pub fn set_args(&mut self, args: &[&str]) {
        self.global.add_list(
            "args",
            args.iter()
                .map(|arg| Value::Str(Rc::new(arg.to_string())))
                .collect::<Vec<_>>(),
        );
    }

    /// Run a script and capture the final value
    pub fn run(&mut self, ast: &Vec<AstNode>) -> Result<Value<'a>, Error> {
        runtime_trace!(self, "run");
        self.value_stack.start_frame();

        self.evaluate_block(ast)?;

        match self.value_stack.values() {
            [] => Ok(Value::Empty),
            [single_value] => Ok(single_value.clone()),
            values @ _ => {
                let list = Value::List(Rc::new(values.to_owned()));
                Ok(list)
            }
        }
    }

    /// Evaluate a series of expressions and keep the final result on the value stack
    fn evaluate_block(&mut self, block: &Vec<AstNode>) -> RuntimeResult {
        runtime_trace!(self, "evaluate_block - {}", block.len());

        self.value_stack.start_frame();

        for (i, expression) in block.iter().enumerate() {
            if i < block.len() - 1 {
                self.evaluate_and_expand(expression)?;
                self.value_stack.pop_frame();
            } else {
                self.evaluate_and_capture(expression)?;
                self.value_stack.pop_frame_and_keep_results();
            }
        }

        Ok(())
    }

    /// Evaluate a series of expressions and add their results to the value stack
    fn evaluate_expressions(&mut self, expressions: &Vec<AstNode>) -> RuntimeResult {
        runtime_trace!(self, "evaluate_expressions - {}", expressions.len());

        self.value_stack.start_frame();

        for expression in expressions.iter() {
            if koto_parser::is_single_value_node(&expression.node) {
                self.evaluate(expression)?;
                self.value_stack.pop_frame_and_keep_results();
            } else {
                self.evaluate_and_capture(expression)?;
                self.value_stack.pop_frame_and_keep_results();
            }
        }

        Ok(())
    }

    /// Evaluate an expression and capture multiple return values in a List
    ///
    /// Single return values get left on the stack without allocation
    fn evaluate_and_capture(&mut self, expression: &AstNode) -> RuntimeResult {
        use Value::*;

        runtime_trace!(self, "evaluate_and_capture - {}", expression.node);

        self.value_stack.start_frame();

        if koto_parser::is_single_value_node(&expression.node) {
            self.evaluate(expression)?;
            self.value_stack.pop_frame_and_keep_results();
        } else {
            self.evaluate_and_expand(expression)?;

            match self.value_stack.value_count() {
                0 => {
                    self.value_stack.pop_frame();
                    self.value_stack.push(Empty);
                }
                1 => {
                    self.value_stack.pop_frame_and_keep_results();
                }
                _ => {
                    let list = self
                        .value_stack
                        .values()
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
                    self.value_stack.pop_frame();
                    self.value_stack.push(List(Rc::new(list)));
                }
            }
        }

        Ok(())
    }

    /// Evaluates a single expression, and expands single return values
    ///
    /// A single For loop or Range in first position will be expanded
    fn evaluate_and_expand(&mut self, expression: &AstNode) -> RuntimeResult {
        use Value::*;

        runtime_trace!(self, "evaluate_and_expand - {}", expression.node);

        self.value_stack.start_frame();

        self.evaluate(expression)?;

        if self.value_stack.values().len() == 1 {
            let expand_value = match self.value_stack.value() {
                For(_) | Range { .. } => true,
                _ => false,
            };

            if expand_value {
                let value = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                match value {
                    For(_) => {
                        self.run_for_loop(&value, expression)?;
                        let loop_value_count = self.value_stack.value_count();
                        match loop_value_count {
                            0 => {
                                self.value_stack.pop_frame();
                                self.value_stack.push(Empty);
                            }
                            1 => {
                                self.value_stack.pop_frame_and_keep_results();
                            }
                            _ => {
                                self.value_stack.pop_frame_and_keep_results();
                            }
                        }
                    }
                    Range { min, max } => {
                        for i in min..max {
                            self.value_stack.push(Number(i as f64))
                        }
                    }
                    _ => unreachable!(),
                }
            } else {
                self.value_stack.pop_frame_and_keep_results();
            }
        } else {
            self.value_stack.pop_frame_and_keep_results();
        }

        Ok(())
    }

    fn evaluate(&mut self, node: &AstNode) -> RuntimeResult {
        runtime_trace!(self, "evaluate - {}", node.node);

        self.value_stack.start_frame();

        use Value::*;

        match &node.node {
            Node::Bool(b) => {
                self.value_stack.push(Bool(*b));
            }
            Node::Number(n) => {
                self.value_stack.push(Number(*n));
            }
            Node::Vec4(v) => {
                self.value_stack.push(Vec4(*v));
            }
            Node::Str(s) => {
                self.value_stack.push(Str(s.clone()));
            }
            Node::List(elements) => {
                self.evaluate_expressions(elements)?;
                if self.value_stack.values().len() == 1 {
                    let value = self.value_stack.value().clone();
                    self.value_stack.pop_frame();
                    match value {
                        List(_) => self.value_stack.push(value),
                        _ => self.value_stack.push(List(Rc::new(vec![value]))),
                    }
                } else {
                    // TODO check values in value stack for unexpanded for loops + ranges
                    let list = self
                        .value_stack
                        .values()
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    self.value_stack.pop_frame();
                    self.value_stack.push(Value::List(Rc::new(list)));
                }
            }
            Node::Range {
                min,
                inclusive,
                max,
            } => {
                self.evaluate(min)?;
                let min = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                self.evaluate(max)?;
                let max = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                match (min, max) {
                    (Number(min), Number(max)) => {
                        let min = min as isize;
                        let max = max as isize;
                        let max = if *inclusive { max + 1 } else { max };
                        if min <= max {
                            self.value_stack.push(Range { min, max });
                        } else {
                            return runtime_error!(
                                node,
                                "Invalid range, min should be less than or equal to max - min: {}, max: {}",
                                min,
                                max);
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
            Node::Map(entries) => {
                let mut map = ValueMap::with_capacity(entries.len());
                for (id, node) in entries.iter() {
                    self.evaluate_and_capture(node)?;
                    map.insert(id.clone(), self.value_stack.value().clone());
                    self.value_stack.pop_frame();
                }
                self.value_stack.push(Map(Rc::new(RefCell::new(map))));
            }
            Node::Index(index) => {
                self.list_index(&index.id, &index.expression, node)?;
            }
            Node::Id(id) => {
                self.value_stack
                    .push(self.get_value_or_error(&id.as_slice(), node)?.0);
            }
            Node::RefId(id) => {
                self.make_reference(id, node)?;
            }
            Node::Block(block) => {
                self.evaluate_block(&block)?;
                self.value_stack.pop_frame_and_keep_results();
            }
            Node::Expressions(expressions) => {
                self.evaluate_expressions(&expressions)?;
                self.value_stack.pop_frame_and_keep_results();
            }
            Node::RefExpression(expression) => {
                self.evaluate_and_capture(expression)?;
                let value = self.value_stack.value().clone();
                self.value_stack.pop_frame();
                match value {
                    Ref(_) => self.value_stack.push(value),
                    _ => self.value_stack.push(Ref(Rc::new(RefCell::new(value)))),
                };
            }
            Node::Function(f) => self.value_stack.push(Function(f.clone())),
            Node::Call { function, args } => {
                return self.call_function(function, args, node);
            }
            Node::Assign { target, expression } => {
                self.evaluate_and_capture(expression)?;

                let value = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                match target {
                    AssignTarget::Id { id, scope } => {
                        self.set_value(id, value.clone(), *scope);
                    }
                    AssignTarget::Index(AstIndex { id, expression }) => {
                        self.set_list_value(id, expression, value.clone(), node)?;
                    }
                    AssignTarget::Lookup(lookup) => {
                        self.set_map_value(lookup, value.clone(), node)?;
                    }
                }

                self.value_stack.push(value);
            }
            Node::MultiAssign {
                targets,
                expressions,
            } => {
                macro_rules! set_value {
                    ($target:expr, $value:expr) => {
                        match $target {
                            AssignTarget::Id { id, scope } => {
                                self.set_value(&id, $value, *scope);
                            }
                            AssignTarget::Index(AstIndex { id, expression }) => {
                                self.set_list_value(&id, &expression, $value, node)?;
                            }
                            AssignTarget::Lookup(lookup) => {
                                self.set_map_value(lookup, $value.clone(), node)?;
                            }
                        }
                    };
                };

                if expressions.len() == 1 {
                    self.evaluate_and_capture(expressions.first().unwrap())?;
                    let value = self.value_stack.value().clone();
                    self.value_stack.pop_frame_and_keep_results();

                    match value {
                        List(l) => {
                            let mut result_iter = l.iter();
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
                            set_value!(first_id, value);

                            for id in targets[1..].iter() {
                                set_value!(id, Empty);
                            }
                        }
                    }
                } else {
                    for expression in expressions.iter() {
                        self.evaluate_and_capture(expression)?;
                        self.value_stack.pop_frame_and_keep_results();
                    }

                    let results = self.value_stack.values().to_owned();

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
                }
            }
            Node::Op { op, lhs, rhs } => {
                self.evaluate(lhs)?;
                let a = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                self.evaluate(rhs)?;
                let b = self.value_stack.value().clone();
                self.value_stack.pop_frame();

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

                let result = match op {
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
                                let mut result = Vec::clone(a);
                                result.extend(Vec::clone(b).into_iter());
                                Ok(List(Rc::new(result)))
                            }
                            _ => binary_op_error!(op, a, b),
                        },
                        (Map(a), Map(b)) => match op {
                            AstOp::Add => {
                                let mut result = a.borrow().0.clone();
                                result.extend(b.borrow().0.clone().into_iter());
                                Ok(Map(Rc::new(RefCell::new(ValueMap(result)))))
                            }
                            _ => binary_op_error!(op, a, b),
                        },
                        _ => binary_op_error!(op, a, b),
                    },
                }?;

                self.value_stack.push(result);
            }
            Node::If {
                condition,
                then_node,
                else_if_condition,
                else_if_node,
                else_node,
            } => {
                self.evaluate(condition)?;
                let maybe_bool = self.value_stack.value().clone();
                self.value_stack.pop_frame();

                if let Bool(condition_value) = maybe_bool {
                    if condition_value {
                        self.evaluate(then_node)?;
                        self.value_stack.pop_frame_and_keep_results();
                        return Ok(());
                    }

                    if else_if_condition.is_some() {
                        self.evaluate(&else_if_condition.as_ref().unwrap())?;
                        let maybe_bool = self.value_stack.value().clone();
                        self.value_stack.pop_frame();

                        if let Bool(condition_value) = maybe_bool {
                            if condition_value {
                                self.evaluate(else_if_node.as_ref().unwrap())?;
                                self.value_stack.pop_frame_and_keep_results();
                                return Ok(());
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
                        self.evaluate(else_node.as_ref().unwrap())?;
                        self.value_stack.pop_frame_and_keep_results();
                    }
                } else {
                    return runtime_error!(
                        node,
                        "Expected bool in if statement, found {}",
                        type_as_string(&maybe_bool)
                    );
                }
            }
            Node::For(f) => {
                self.value_stack.push(For(f.clone()));
            }
        }

        Ok(())
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
        lookup_id: &LookupIdSlice,
        node: &AstNode,
        mut visitor: impl FnMut(&LookupIdSlice, &AstNode, &mut Value<'a>) -> RuntimeResult + Clone + 'b,
    ) -> RuntimeResult {
        macro_rules! value_or_map_lookup {
            ($value:expr) => {{
                match $value {
                    Some(value) => {
                        if lookup_id.0.len() == 1 {
                            return visitor(lookup_id, node, value);
                        } else if let Value::Map(map) = value {
                            let (found, error) =
                                map.borrow_mut()
                                    .visit_mut(lookup_id, 1, node, visitor.clone());
                            (found, Some(error))
                        } else {
                            (false, None)
                        }
                    }
                    _ => (false, None),
                }
            }};
        }

        let first_id = lookup_id.0.first().unwrap();

        if self.call_stack.frame() > 0 {
            let value = self.call_stack.get_mut(first_id);
            match value_or_map_lookup!(value) {
                (false, _) => {}
                (true, Some(result)) => {
                    return result;
                }
                _ => unreachable!(),
            }
        }

        let global_value = self.global.0.get_mut(first_id);
        match value_or_map_lookup!(global_value) {
            (false, None) => runtime_error!(node, "'{}' not found", lookup_id),
            (false, Some(result)) => result,
            (true, Some(result)) => result,
            _ => unreachable!(),
        }
    }

    fn get_value(&self, lookup_id: &LookupIdSlice) -> Option<(Value<'a>, Scope)> {
        macro_rules! value_or_map_lookup {
            ($value:expr) => {{
                if lookup_id.0.len() == 1 {
                    $value
                } else if $value.is_some() {
                    let mut result = $value;
                    for id in lookup_id.0[1..].iter() {
                        // TODO simplify via ValueMap
                        if result.is_none() {
                            break;
                        }
                        result = match result {
                            Some(Value::Map(data)) => data.borrow().0.get(id).map(|v| v.clone()),
                            _unexpected => None, // TODO error, previous item wasn't a map
                        };
                    }
                    result
                } else {
                    None
                }
            }};
        }

        let first_id = lookup_id.0.first().unwrap();
        if self.call_stack.frame() > 0 {
            let value = self.call_stack.get(first_id).map(|v| v.clone());
            if let Some(value) = value_or_map_lookup!(value) {
                return Some((value, Scope::Local));
            }
        }

        let global_value = self.global.0.get(first_id).map(|v| v.clone());
        value_or_map_lookup!(global_value).map(|v| (v, Scope::Global))
    }

    fn get_value_or_error(
        &self,
        id: &LookupIdSlice,
        node: &AstNode,
    ) -> Result<(Value<'a>, Scope), Error> {
        match self.get_value(id) {
            Some(v) => Ok(v),
            None => runtime_error!(node, "'{}' not found", id),
        }
    }

    fn run_for_loop(&mut self, for_statement: &Value<'a>, node: &AstNode) -> RuntimeResult {
        runtime_trace!(self, "run_for_loop");
        use Value::*;

        self.value_stack.start_frame();

        if let For(f) = for_statement {
            if f.ranges.len() == 1 {
                self.evaluate(f.ranges.first().unwrap())?;
                let range = self.value_stack.value().clone();
                self.value_stack.pop_frame();

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
                                for list_value in a.iter() {
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
                        self.evaluate(&condition)?;
                        let value = self.value_stack.value().clone();
                        self.value_stack.pop_frame();

                        match value {
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
                    self.evaluate_and_capture(&f.body)?;
                    self.value_stack.pop_frame_and_keep_results();
                }
            } else {
                self.multi_range_iterator.iterators.clear();
                for range in f.ranges.iter() {
                    self.evaluate(range)?;
                    let range = self.value_stack.value().clone();
                    self.value_stack.pop_frame();

                    match deref_value(&range) {
                        v @ List(_) | v @ Range { .. } => self
                            .multi_range_iterator
                            .iterators
                            .push(ValueIterator::new(v)),
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

                while self
                    .multi_range_iterator
                    .push_next_values_to_stack(&mut self.value_stack)
                {
                    if single_arg {
                        if self.value_stack.value_count() == 1 {
                            let value = self.value_stack.value().clone();
                            self.set_value(first_arg, value, Scope::Local);
                            self.value_stack.pop_frame();
                        } else {
                            let values = self
                                .value_stack
                                .values()
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>();
                            self.set_value(first_arg, Value::List(Rc::new(values)), Scope::Local);
                        }
                    } else {
                        let mut arg_iter = f.args.iter().peekable();
                        for i in 0..self.value_stack.value_count() {
                            match arg_iter.next() {
                                Some(arg) => {
                                    let value = self.value_stack.values()[i].clone();
                                    self.set_value(arg, value.clone(), Scope::Local);
                                }
                                None => break,
                            }
                        }
                        for remaining_arg in arg_iter {
                            self.set_value(remaining_arg, Value::Empty, Scope::Local);
                        }
                        self.value_stack.pop_frame();
                    }

                    if let Some(condition) = &f.condition {
                        self.evaluate(&condition)?;
                        let value = self.value_stack.value().clone();
                        self.value_stack.pop_frame();

                        match value {
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
                    self.evaluate_and_capture(&f.body)?;
                    self.value_stack.pop_frame_and_keep_results();
                }
            }
        }

        Ok(())
    }

    fn set_map_value(&mut self, id: &LookupId, value: Value<'a>, node: &AstNode) -> RuntimeResult {
        let value_id = id.0.last().unwrap().clone();

        self.visit_value_mut(&id.map_slice(), node, move |map_id, node, maybe_map| {
            if let Value::Map(map) = maybe_map {
                Rc::make_mut(map)
                    .borrow_mut()
                    .add_value(&value_id, value.clone());
                Ok(())
            } else {
                runtime_error!(
                    node,
                    "Expected Map for '{}', found {}",
                    map_id,
                    type_as_string(&maybe_map)
                )
            }
        })
    }

    fn set_list_value(
        &mut self,
        id: &LookupId,
        expression: &AstNode,
        value: Value<'a>,
        node: &AstNode,
    ) -> RuntimeResult {
        use Value::*;

        self.evaluate(expression)?;
        let index = self.value_stack.value().clone();
        self.value_stack.pop_frame();

        self.visit_value_mut(&id.as_slice(), node, move |id, node, maybe_list| {
            let assign_to_index = |data: &mut Vec<Value<'a>>| match index {
                Number(i) => {
                    let i = i as usize;
                    if i < data.len() {
                        data[i] = value.clone();
                        Ok(())
                    } else {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} but the index is {}",
                            id,
                            data.len(),
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
                    } else if umin >= data.len() || umax > data.len() {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                            id,
                            data.len(),
                            min,
                            max
                        )
                    } else {
                        for element in &mut data[umin..umax] {
                            *element = value.clone();
                        }
                        Ok(())
                    }
                }
                _ => runtime_error!(
                    node,
                    "Indexing is only supported with number values or ranges, found {})",
                    type_as_string(&index)
                ),
            };

            match maybe_list {
                List(data) => assign_to_index(&mut Rc::make_mut(data)),
                Ref(r) => match *r.borrow_mut() {
                    List(ref mut data) => assign_to_index(&mut Rc::make_mut(data)),
                    _ => runtime_error!(
                        node,
                        "Indexing is only supported for Lists" // TODO, improve error
                    ),
                },
                _ => runtime_error!(
                    node,
                    "Indexing is only supported for Lists, found {}",
                    type_as_string(&maybe_list)
                ),
            }
        })
    }

    fn list_index(&mut self, id: &LookupId, expression: &AstNode, node: &AstNode) -> RuntimeResult {
        use Value::*;

        self.evaluate(expression)?;
        let index = self.value_stack.value().clone();
        self.value_stack.pop_frame();

        let (maybe_list, _scope) = self.get_value_or_error(&id.as_slice(), node)?;

        if let List(elements) = deref_value(&maybe_list) {
            match index {
                Number(i) => {
                    let i = i as usize;
                    if i < elements.len() {
                        self.value_stack.push(elements[i].clone());
                    } else {
                        return runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} but the index is {}",
                            id,
                            elements.len(),
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
                    } else if umin >= elements.len() || umax >= elements.len() {
                        return runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                            id,
                            elements.len(),
                            min,
                            max
                        );
                    } else {
                        // TODO Avoid allocating new vec, introduce 'slice' value type
                        self.value_stack.push(List(Rc::new(
                            elements[umin..umax].iter().cloned().collect::<Vec<_>>(),
                        )));
                    }
                }
                _ => {
                    return runtime_error!(
                        node,
                        "Indexing is only supported with number values or ranges, found {})",
                        type_as_string(&index)
                    )
                }
            }
        } else {
            return runtime_error!(
                node,
                "Indexing is only supported for Lists, found {}",
                type_as_string(&maybe_list)
            );
        }

        Ok(())
    }

    fn call_function(
        &mut self,
        id: &LookupId,
        args: &Vec<AstNode>,
        node: &AstNode,
    ) -> RuntimeResult {
        use Value::*;

        runtime_trace!(self, "call_function - {}", id);

        let maybe_function = match self.get_value(&id.as_slice()) {
            Some((ExternalFunction(f), _)) => {
                self.evaluate_expressions(args)?;
                let mut closure = f.0.borrow_mut();
                let builtin_result = (&mut *closure)(&self.value_stack.values());
                self.value_stack.pop_frame();
                return match builtin_result {
                    Ok(v) => {
                        self.value_stack.push(v);
                        Ok(())
                    }
                    Err(e) => runtime_error!(node, e),
                };
            }
            Some((Function(f), _)) => Some(f.clone()),
            Some((unexpected, _)) => {
                return runtime_error!(
                    node,
                    "Expected function for value {}, found {}",
                    id,
                    type_as_string(&unexpected)
                )
            }
            None => None,
        };

        if let Some(f) = maybe_function {
            let arg_count = f.args.len();
            let expected_args =
                if id.0.len() > 1 && arg_count > 0 && f.args.first().unwrap().as_ref() == "self" {
                    arg_count - 1
                } else {
                    arg_count
                };

            if args.len() != expected_args {
                return runtime_error!(
                    node,
                    "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                    id,
                    expected_args,
                    args.len(),
                    f.args
                );
            }

            // allow the function that's being called to call itself
            self.call_stack
                .push(id.0.first().unwrap().clone(), Function(f.clone()));

            // implicit self for map functions
            if id.0.len() > 1 {
                match f.args.first() {
                    Some(self_arg) if self_arg.as_ref() == "self" => {
                        let (map, _scope) = self.get_value(&id.map_slice()).unwrap();
                        self.call_stack.push(self_arg.clone(), map);
                    }
                    _ => {}
                }
            }

            for (name, arg) in f.args.iter().zip(args.iter()) {
                let expression_result = self.evaluate_and_capture(arg);

                if expression_result.is_err() {
                    self.value_stack.pop_frame();
                    self.call_stack.cancel();
                    return expression_result;
                }

                let arg_value = self.value_stack.value().clone();
                self.value_stack.pop_frame();
                self.call_stack.push(name.clone(), arg_value);
            }

            self.call_stack.commit();
            let result = self.evaluate_block(&f.body);
            self.value_stack.pop_frame_and_keep_results();
            self.call_stack.pop_frame();

            return result;
        }

        runtime_error!(node, "Function '{}' not found", id)
    }

    fn make_reference(&mut self, id: &LookupId, node: &AstNode) -> RuntimeResult {
        if id.0.len() == 1 {
            let (value, scope) = self.get_value_or_error(&id.as_slice(), node)?;
            let value_ref = match value {
                Value::Ref(_) => {
                    self.value_stack.push(value.clone());
                    return Ok(());
                }
                _ => Value::Ref(Rc::new(RefCell::new(value.clone()))),
            };
            self.set_value(&id.0[0], value_ref.clone(), scope);
            self.value_stack.push(value_ref);
            Ok(())
        } else {
            unimplemented!();
        }
    }

    pub fn global_mut(&mut self) -> &mut ValueMap<'a> {
        return &mut self.global;
    }

    #[allow(dead_code)]
    fn runtime_indent(&self) -> String {
        " ".repeat(self.value_stack.frame_count())
    }
}
