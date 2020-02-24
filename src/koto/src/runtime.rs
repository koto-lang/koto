#![macro_use]

use koto_parser::{AstNode, AstOp, Node, Position};
use std::{collections::HashMap, fmt, rc::Rc};

use crate::{
    callstack::CallStack,
    value::{MultiRangeValueIterator, Value, ValueIterator},
    Id, LookupId,
};

#[derive(Debug)]
pub enum Error {
    RuntimeError {
        message: String,
        start_pos: Position,
        end_pos: Position,
    },
}

pub type RuntimeResult = Result<Value, Error>;
pub type BuiltinResult = Result<Value, String>;

#[derive(Debug)]
pub struct Scope {
    values: HashMap<Rc<String>, Value>,
}

impl Scope {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    fn print_keys(&self) {
        println!(
            "{:?}",
            self.values
                .keys()
                .map(|key| key.as_ref().clone())
                .collect::<Vec<_>>()
        );
    }
}

macro_rules! make_runtime_error {
    ($node:expr, $message:expr) => {
        Error::RuntimeError {
            message: $message,
            start_pos: $node.start_pos,
            end_pos: $node.end_pos,
        }
    };
}

macro_rules! runtime_error {
    ($node:expr, $error:expr) => {
        Err(make_runtime_error!($node, String::from($error)))
    };
    ($node:expr, $error:expr, $($y:expr),+) => {
        Err(make_runtime_error!($node, format!($error, $($y),+)))
    };
}

pub type BuiltinFunction<'a> = Box<dyn FnMut(&Vec<Value>) -> BuiltinResult + 'a>;

pub enum BuiltinValue<'a> {
    Function(BuiltinFunction<'a>),
    Map(BuiltinMap<'a>),
}

impl<'a> fmt::Display for BuiltinValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BuiltinValue::*;
        match self {
            Function(_) => write!(f, "Builtin Function"),
            Map(_) => write!(f, "Builtin Map"),
        }
    }
}

pub struct BuiltinMap<'a>(HashMap<String, BuiltinValue<'a>>);

impl<'a> BuiltinMap<'a> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add_map(&mut self, name: &str) -> &mut BuiltinMap<'a> {
        self.0
            .insert(name.to_string(), BuiltinValue::Map(BuiltinMap::new()));

        if let BuiltinValue::Map(map) = self.0.get_mut(name).unwrap() {
            return map;
        }

        unreachable!();
    }

    pub fn add_fn(&mut self, name: &str, f: impl FnMut(&Vec<Value>) -> BuiltinResult + 'a) {
        self.0
            .insert(name.to_string(), BuiltinValue::Function(Box::new(f)));
    }

    pub fn get_mut(&mut self, lookup_id: &[Id]) -> Option<&mut BuiltinValue<'a>> {
        use BuiltinValue::*;

        match self.0.get_mut(lookup_id.first().unwrap().as_ref()) {
            Some(value) => {
                if lookup_id.len() == 1 {
                    Some(value)
                } else {
                    match value {
                        Map(map) => map.get_mut(&lookup_id[1..]),
                        Function(_) => None,
                    }
                }
            }
            None => None,
        }
    }
}

pub struct Runtime<'a> {
    global: Scope,
    builtins: BuiltinMap<'a>,
    callstack: CallStack,
}

impl<'a> Runtime<'a> {
    pub fn new() -> Self {
        let mut result = Self {
            global: Scope::new(),
            builtins: BuiltinMap::new(),
            callstack: CallStack::new(),
        };
        crate::builtins::register(&mut result);
        result
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> RuntimeResult {
        self.evaluate_block(ast)
    }

    pub fn builtins_mut(&mut self) -> &mut BuiltinMap<'a> {
        return &mut self.builtins;
    }

    fn evaluate_block(&mut self, block: &Vec<AstNode>) -> RuntimeResult {
        let mut result = Value::Empty;
        for (i, node) in block.iter().enumerate() {
            let output = self.evaluate(node)?;
            match output {
                Value::For(_) => {
                    if i < block.len() - 1 {
                        self.run_for_loop(output, node, &mut None)?;
                    } else {
                        let mut loop_output = Vec::new(); // TODO use return stack
                        self.run_for_loop(output, node, &mut Some(&mut loop_output))?;
                        if !loop_output.is_empty() {
                            result = Value::List(Rc::new(loop_output))
                        }
                    }
                }
                _ => result = output,
            }
        }
        Ok(result)
    }

    fn evaluate_expressions(&mut self, expressions: &Vec<AstNode>) -> RuntimeResult {
        let result = expressions
            .iter()
            .map(|expression| self.evaluate_and_capture(expression))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Value::List(Rc::new(result)))
    }

    fn evaluate_and_capture(&mut self, node: &AstNode) -> RuntimeResult {
        use Value::*;
        match self.evaluate(node)? {
            for_loop @ For(_) => {
                let mut loop_output = Vec::new();
                self.run_for_loop(for_loop, node, &mut Some(&mut loop_output))?;
                Ok(List(Rc::new(loop_output)))
            }
            // Range { min, max } => {
            //     let mut range_output = Vec::new();
            //     for i in min..max {
            //         range_output.push(Number(i as f64))
            //     }
            //     Ok(List(Rc::new(range_output)))
            // }
            // Empty => {}
            result => Ok(result),
        }
    }

    fn evaluate(&mut self, node: &AstNode) -> RuntimeResult {
        use Value::*;

        match &node.node {
            Node::Bool(b) => Ok(Bool(*b)),
            Node::Number(n) => Ok(Number(*n)),
            Node::Str(s) => Ok(Str(s.clone())),
            Node::List(elements) => {
                let mut values = Vec::new(); // TODO use return stack
                for node in elements.iter() {
                    let value = self.evaluate(node)?;
                    match value {
                        Range { min, max } => {
                            for i in min..max {
                                values.push(Number(i as f64))
                            }
                        }
                        Value::For(_) => {
                            self.run_for_loop(value, node, &mut Some(&mut values))?;
                        }
                        _ => values.push(value),
                    }
                }
                Ok(List(Rc::new(values)))
            }
            Node::Range {
                min,
                inclusive,
                max,
            } => {
                let min = self.evaluate(min)?;
                let max = self.evaluate(max)?;
                match (min, max) {
                    (Number(min), Number(max)) => {
                        let min = min as isize;
                        let max = max as isize;
                        let max = if *inclusive { max + 1 } else { max };
                        if min <= max {
                            Ok(Range { min, max })
                        } else {
                            runtime_error!(
                                node,
                                "Invalid range, min should be less than or equal to max - min: {}, max: {}",
                                min,
                                max)
                        }
                    }
                    unexpected => runtime_error!(
                        node,
                        "Expected numbers for range bounds, found min: {}, max: {}",
                        unexpected.0,
                        unexpected.1
                    ),
                }
            }
            Node::Map(entries) => {
                let mut map = HashMap::new();
                for (id, node) in entries.iter() {
                    map.insert(id.clone(), self.evaluate_and_capture(node)?);
                }
                Ok(Map(Rc::new(map)))
            }
            Node::Index { id, expression } => self.list_index(id, expression, node),
            Node::Id(id) => self.get_value_or_error(id, node),
            Node::Block(block) => self.evaluate_block(&block),
            Node::Expressions(expressions) => self.evaluate_expressions(&expressions),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => {
                let result = self.call_function(function, args, node);
                // println!("Called {}, returning {:?}", function, result);
                result
            }
            Node::Assign { id, expression } => {
                let value = self.evaluate_and_capture(expression)?;
                self.set_value(id, &value);
                Ok(value)
            }
            Node::MultiAssign { ids, expressions } => {
                let mut id_iter = ids.iter().peekable();
                let mut expressions_iter = expressions.iter();
                let mut result = vec![];
                while id_iter.peek().is_some() {
                    match expressions_iter.next() {
                        Some(expression) => match self.evaluate(expression)? {
                            List(a) => {
                                for value in a.iter() {
                                    match id_iter.next() {
                                        Some(id) => {
                                            result.push(value.clone());
                                            self.set_value(id, &value)
                                        }
                                        None => break,
                                    }
                                }
                            }
                            value => {
                                result.push(value.clone());
                                self.set_value(id_iter.next().unwrap(), &value)
                            }
                        },
                        None => self.set_value(id_iter.next().unwrap(), &Value::Empty),
                    }
                }
                Ok(List(Rc::new(result)))
            }
            Node::Op { op, lhs, rhs } => {
                // dbg!(lhs);
                // dbg!(ops);
                let a = self.evaluate(lhs)?;
                let b = self.evaluate(rhs)?;
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
                                let mut result = HashMap::clone(a);
                                result.extend(HashMap::clone(b).into_iter());
                                Ok(Map(Rc::new(result)))
                            }
                            _ => binary_op_error!(op, a, b),
                        },
                        _ => binary_op_error!(op, a, b),
                    },
                }
            }
            Node::If {
                condition,
                then_node,
                else_if_condition,
                else_if_node,
                else_node,
            } => {
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
                                maybe_bool
                            );
                        }
                    }

                    if else_node.is_some() {
                        self.evaluate(else_node.as_ref().unwrap())
                    } else {
                        Ok(Value::Empty)
                    }
                } else {
                    runtime_error!(node, "Expected bool in if statement, found {}", maybe_bool)
                }
            }
            Node::For(f) => Ok(For(f.clone())),
        }
    }

    fn set_value(&mut self, id: &Id, value: &Value) {
        if self.callstack.frame() > 0 {
            if let Some(exists) = self.callstack.get_mut(id.as_ref()) {
                *exists = value.clone();
            } else {
                self.callstack.extend(id.clone(), value.clone());
            }
        } else {
            self.global.values.insert(id.clone(), value.clone());
        }
    }

    fn get_value(&self, lookup_id: &LookupId) -> Option<Value> {
        macro_rules! value_or_map_lookup {
            ($value:expr) => {{
                if lookup_id.0.len() == 1 {
                    $value
                } else if $value.is_some() {
                    lookup_id.0[1..]
                        .iter()
                        .try_fold($value.unwrap(), |result, id| {
                            match result {
                                Value::Map(data) => data.get(id),
                                _unexpected => None, // TODO error, previous item wasn't a map
                            }
                        })
                } else {
                    None
                }
            }};
        }

        if self.callstack.frame() > 0 {
            let value = self.callstack.get(lookup_id.0.first().unwrap());
            if let Some(value) = value_or_map_lookup!(value) {
                return Some(value.clone());
            }
        }

        let global_value = self.global.values.get(lookup_id.0.first().unwrap());
        value_or_map_lookup!(global_value).map(|v| v.clone())
    }

    fn get_value_or_error(&self, id: &LookupId, node: &AstNode) -> RuntimeResult {
        match self.get_value(id) {
            Some(v) => Ok(v),
            None => runtime_error!(node, "Value '{}' not found", id),
        }
    }

    fn run_for_loop(
        &mut self,
        for_statement: Value,
        node: &AstNode,
        collector: &mut Option<&mut Vec<Value>>,
    ) -> RuntimeResult {
        use Value::*;
        let mut result = Value::Empty;

        if let Value::For(f) = for_statement {
            let iter = MultiRangeValueIterator(
                f.ranges
                    .iter()
                    .map(|range| match self.evaluate(range)? {
                        v @ List(_) | v @ Range { .. } => Ok(ValueIterator::new(v)),
                        unexpected => runtime_error!(
                            node,
                            "Expected iterable range in for statement, found {}",
                            unexpected
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );

            let single_range = f.ranges.len() == 1;
            for values in iter {
                let mut arg_iter = f.args.iter().peekable();
                for value in values.iter() {
                    match value {
                        List(a) if single_range => {
                            for list_value in a.iter() {
                                match arg_iter.next() {
                                    Some(arg) => self.set_value(arg, &list_value), // TODO
                                    None => break,
                                }
                            }
                        }
                        _ => self.set_value(
                            arg_iter
                                .next()
                                .expect("For loops have at least one argument"),
                            &value,
                        ),
                    }
                }
                for remaining_arg in arg_iter {
                    self.set_value(remaining_arg, &Value::Empty);
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
                                unexpected
                            )
                        }
                    }
                }

                result = self.evaluate(&f.body)?;
                match result {
                    Empty => {}
                    _ => {
                        if let Some(collector) = collector.as_mut() {
                            collector.push(result.clone());
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn list_index(&mut self, id: &LookupId, expression: &AstNode, node: &AstNode) -> RuntimeResult {
        use Value::*;

        let index = self.evaluate(expression)?;
        let maybe_list = self.get_value_or_error(id, node)?;

        if let List(elements) = maybe_list {
            match index {
                Number(i) => {
                    let i = i as usize;
                    if i < elements.len() {
                        Ok(elements[i].clone())
                    } else {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} but the index is {}",
                            id,
                            elements.len(),
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
                    } else if umin >= elements.len() || umax >= elements.len() {
                        runtime_error!(
                            node,
                            "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                            id,
                            elements.len(),
                            min,
                            max
                        )
                    } else {
                        Ok(List(Rc::new(
                            elements[umin..umax].iter().cloned().collect::<Vec<_>>(),
                        )))
                    }
                }
                _ => runtime_error!(
                    node,
                    "Indexing is only supported with number values or ranges, found {})",
                    index
                ),
            }
        } else {
            runtime_error!(
                node,
                "Indexing is only supported for Lists, found {}",
                maybe_list
            )
        }
    }

    fn call_function(
        &mut self,
        id: &LookupId,
        args: &Vec<AstNode>,
        node: &AstNode,
    ) -> RuntimeResult {
        use Value::*;

        let maybe_function = match self.get_value(id) {
            Some(Function(f)) => Some(f.clone()),
            Some(unexpected) => {
                return runtime_error!(
                    node,
                    "Expected function for value {}, found {}",
                    id,
                    unexpected
                )
            }
            None => None,
        };

        if let Some(f) = maybe_function {
            let arg_count = f.args.len();
            if args.len() != arg_count {
                return runtime_error!(
                    node,
                    "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                    id,
                    arg_count,
                    args.len(),
                    f.args
                );
            }

            // allow the function that's being called to call itself
            self.callstack
                .push(id.0.first().unwrap().clone(), Function(f.clone()));

            for (name, arg) in f.args.iter().zip(args.iter()) {
                match self.evaluate_and_capture(arg) {
                    Ok(value) => self.callstack.push(name.clone(), value),
                    Err(e) => {
                        self.callstack.cancel();
                        return Err(e);
                    }
                };
            }

            self.callstack.commit();
            let result = self.evaluate_block(&f.body);
            self.callstack.pop_frame();

            return result;
        }

        let arg_values = args
            .iter()
            .map(|arg| self.evaluate_and_capture(arg))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(value) = self.builtins.get_mut(&id.0) {
            return match value {
                BuiltinValue::Function(f) => match f(&arg_values) {
                    Ok(v) => Ok(v),
                    Err(e) => runtime_error!(node, e),
                },
                unexpected => {
                    runtime_error!(node, "Expected function for '{}', found {}", id, unexpected)
                }
            };
        }

        runtime_error!(node, "Function '{}' not found", id)
    }
}
