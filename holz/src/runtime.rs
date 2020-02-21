use std::{collections::HashMap, fmt, rc::Rc};

use crate::parser::{AstFor, AstNode, AstOp, Function, Node, Position};

pub enum Error {
    RuntimeError {
        message: String,
        start_pos: Position,
        end_pos: Position,
    },
}

pub type RuntimeResult = Result<Value, Error>;

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Array(Rc<Vec<Value>>),
    Range { min: isize, max: isize },
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
    For(Rc<AstFor>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Empty => write!(f, "()"),
            Bool(s) => write!(f, "{}", s),
            Number(n) => write!(f, "{}", n),
            StrLiteral(s) => write!(f, "{}", s),
            Array(a) => {
                write!(f, "[")?;
                for (i, value) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Range { min, max } => write!(f, "[{}..{}]", min, max),
            Function(function) => {
                let raw = Rc::into_raw(function.clone());
                write!(f, "function: {:?}", raw)
            }
            _ => unreachable!(),
        }
    }
}

struct ValueIterator {
    value: Value,
    index: isize,
}

impl ValueIterator {
    pub fn new(value: Value) -> Self {
        Self { value, index: 0 }
    }
}

impl Iterator for ValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        use Value::*;

        let result = match &self.value {
            Array(a) => a.get(self.index as usize).cloned(),
            Range { min, max } => {
                if self.index < (max - min) {
                    Some(Number((min + self.index) as f64))
                } else {
                    None
                }
            }
            _ => None,
        };

        if result.is_some() {
            self.index += 1;
        }

        result
    }
}

#[derive(Debug)]
struct Scope {
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

macro_rules! runtime_error {
    ($node:expr, $error:expr) => {
        Err(Error::RuntimeError {
            message: String::from($error),
            start_pos: $node.start_pos,
            end_pos: $node.end_pos,
        })
    };
    ($node:expr, $error:expr, $($y:expr),+) => {
        Err(Error::RuntimeError {
            message: format!($error, $($y),+),
            start_pos: $node.start_pos,
            end_pos: $node.end_pos,
        })
    };
}

pub struct Runtime {
    global: Scope,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            global: Scope::new(),
        }
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> RuntimeResult {
        self.evaluate_block(ast, &mut None)
    }

    fn evaluate_block(&mut self, block: &Vec<AstNode>, scope: &mut Option<Scope>) -> RuntimeResult {
        let mut result = Value::Empty;
        for node in block.iter() {
            let output = self.evaluate(node, scope)?;
            match output {
                Value::For(_) => {
                    result = self.run_for_statement(output, scope, node, &mut None)?;
                }
                _ => result = output,
            }
        }
        Ok(result)
    }

    fn evaluate(&mut self, node: &AstNode, scope: &mut Option<Scope>) -> RuntimeResult {
        use Value::*;

        match &node.node {
            Node::Bool(b) => Ok(Bool(*b)),
            Node::Number(n) => Ok(Number(*n)),
            Node::Str(s) => Ok(StrLiteral(s.clone())),
            Node::Array(elements) => {
                let mut values = Vec::new();
                for node in elements.iter() {
                    let value = self.evaluate(node, scope)?;
                    match value {
                        Range { min, max } => {
                            for i in min..max {
                                values.push(Number(i as f64))
                            }
                        }
                        Value::For(_) => {
                            self.run_for_statement(value, scope, node, &mut Some(&mut values))?;
                        }
                        _ => values.push(value),
                    }
                }
                Ok(Array(Rc::new(values)))
            }
            Node::Range {
                min,
                inclusive,
                max,
            } => {
                let min = self.evaluate(min, scope)?;
                let max = self.evaluate(max, scope)?;
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
            Node::Index { id, expression } => self.array_index(id, expression, scope, node),
            Node::Id(id) => self.get_value_or_error(id, scope, node),
            Node::Block(block) => self.evaluate_block(&block, scope),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => self.call_function(function, args, scope, node),
            Node::Assign { id, expression } => {
                let value = self.evaluate(expression, scope)?;
                self.set_value(id, &value, scope);
                Ok(value)
            }
            Node::MultiAssign { ids, expressions } => {
                let mut id_iter = ids.iter();
                let mut expressions_iter = expressions.iter();
                while let Some(id) = id_iter.next() {
                    match expressions_iter.next() {
                        Some(expression) => {
                            let value = self.evaluate(expression, scope)?;
                            self.set_value(id, &value, scope);
                        }
                        None => return runtime_error!(node, "Missing value to assign to '{}'", id),
                    }
                }
                Ok(Empty) // todo multiple return values
            }
            Node::Op { op, lhs, rhs } => {
                // dbg!(lhs);
                // dbg!(ops);
                let a = self.evaluate(lhs, scope)?;
                let b = self.evaluate(rhs, scope)?;
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
                match (&a, &b) {
                    (Number(a), Number(b)) => match op {
                        AstOp::Add => Ok(Number(a + b)),
                        AstOp::Subtract => Ok(Number(a - b)),
                        AstOp::Multiply => Ok(Number(a * b)),
                        AstOp::Divide => Ok(Number(a / b)),
                        AstOp::Equal => Ok(Bool(a == b)),
                        AstOp::NotEqual => Ok(Bool(a != b)),
                        AstOp::Less => Ok(Bool(a < b)),
                        AstOp::LessOrEqual => Ok(Bool(a <= b)),
                        AstOp::Greater => Ok(Bool(a > b)),
                        AstOp::GreaterOrEqual => Ok(Bool(a >= b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    (Bool(a), Bool(b)) => match op {
                        AstOp::Equal => Ok(Bool(a == b)),
                        AstOp::NotEqual => Ok(Bool(a != b)),
                        AstOp::And => Ok(Bool(*a && *b)),
                        AstOp::Or => Ok(Bool(*a || *b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    _ => binary_op_error!(op, a, b),
                }
            }
            Node::If {
                condition,
                then_node,
                else_node,
            } => {
                let maybe_bool = self.evaluate(condition, scope)?;
                if let Bool(condition_value) = maybe_bool {
                    if condition_value {
                        self.evaluate(then_node, scope)
                    } else if else_node.is_some() {
                        self.evaluate(else_node.as_ref().unwrap(), scope)
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

    fn set_value(&mut self, id: &Rc<String>, value: &Value, scope: &mut Option<Scope>) {
        match scope {
            Some(scope) => scope.values.insert(id.clone(), value.clone()),
            None => self.global.values.insert(id.clone(), value.clone()),
        };
    }

    fn get_value(&self, id: &String, scope: &Option<Scope>) -> Option<Value> {
        if scope.is_some() {
            let scope = scope.as_ref().unwrap();
            if let Some(value) = scope.values.get(id) {
                return Some(value.clone());
            }
        }

        self.global.values.get(id).map(|v| v.clone())
    }

    fn get_value_or_error(
        &self,
        id: &String,
        scope: &Option<Scope>,
        node: &AstNode,
    ) -> RuntimeResult {
        match self.get_value(id, scope) {
            Some(v) => Ok(v),
            None => runtime_error!(node, "Value '{}' not found", id),
        }
    }

    fn run_for_statement(
        &mut self,
        for_statement: Value,
        scope: &mut Option<Scope>,
        node: &AstNode,
        collector: &mut Option<&mut Vec<Value>>,
    ) -> RuntimeResult {
        use Value::*;
        let mut result = Value::Empty;

        if let Value::For(f) = for_statement {
            let iter = match self.evaluate(&f.range, scope)? {
                v @ Array(_) | v @ Range { .. } => ValueIterator::new(v),
                unexpected => {
                    return runtime_error!(
                        node,
                        "Expected iterable range in for statement, found {}",
                        unexpected
                    )
                }
            };

            for value in iter {
                self.set_value(&f.arg, &value, scope);

                if let Some(condition) = &f.condition {
                    match self.evaluate(&condition, scope)? {
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

                result = self.evaluate(&f.body, scope)?;
                if let Some(collector) = collector.as_mut() {
                    collector.push(result.clone());
                }
            }
        }

        Ok(result)
    }

    fn array_index(
        &mut self,
        id: &String,
        expression: &AstNode,
        scope: &mut Option<Scope>,
        node: &AstNode,
    ) -> RuntimeResult {
        use Value::*;

        let index = self.evaluate(expression, scope)?;
        let maybe_array = self.get_value_or_error(id, scope, node)?;

        if let Array(elements) = maybe_array {
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
                        Ok(Array(Rc::new(
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
                "Indexing is only supported for Arrays, found {}",
                maybe_array
            )
        }
    }

    fn call_function(
        &mut self,
        id: &Rc<String>,
        args: &Vec<AstNode>,
        scope: &mut Option<Scope>,
        node: &AstNode,
    ) -> RuntimeResult {
        use Value::*;

        let maybe_function = match self.get_value(id, scope) {
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

            let mut child_scope = Scope::new();

            child_scope.values.insert(id.clone(), Function(f.clone()));

            for (name, arg) in f.args.iter().zip(args.iter()) {
                let arg_value = self.evaluate(arg, scope)?;
                child_scope.values.insert(name.clone(), arg_value);
            }

            return self.evaluate_block(&f.body, &mut Some(child_scope));
        }

        // Builtins, TODO std lib
        let mut arg_values = args.iter().map(|arg| self.evaluate(arg, scope));
        match id.as_str() {
            "assert" => {
                for value in arg_values {
                    match value? {
                        Bool(b) => {
                            if !b {
                                return runtime_error!(node, "Assertion failed");
                            }
                        }
                        _ => {
                            return runtime_error!(
                                node,
                                "assert only expects booleans as arguments"
                            )
                        }
                    }
                }
                Ok(Empty)
            }
            "push" => {
                let first_arg_value = match arg_values.next() {
                    Some(arg) => arg,
                    None => {
                        return runtime_error!(node, "Missing array as first argument for push");
                    }
                };

                match first_arg_value? {
                    Array(array) => {
                        let mut array = array.clone();
                        let array_data = Rc::make_mut(&mut array);
                        for value in arg_values {
                            array_data.push(value?)
                        }
                        Ok(Array(array))
                    }
                    unexpected => {
                        return runtime_error!(
                            node,
                            "push is only supported for arrays, found {}",
                            unexpected
                        )
                    }
                }
            }
            "length" => {
                let first_arg_value = match arg_values.next() {
                    Some(arg) => arg,
                    None => {
                        return runtime_error!(node, "Missing array as argument for length");
                    }
                };
                match first_arg_value? {
                    Array(array) => Ok(Number(array.len() as f64)),
                    Range { min, max } => Ok(Number((max - min) as f64)),
                    unexpected => {
                        return runtime_error!(
                            node,
                            "length is only supported for arrays and ranges, found {}",
                            unexpected
                        )
                    }
                }
            }
            "print" => {
                for value in arg_values {
                    print!("{} ", value?);
                }
                println!();
                Ok(Empty)
            }
            _ => runtime_error!(node, "Function '{}' not found", id.as_str()),
        }
    }
}
