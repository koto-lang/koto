use std::{collections::HashMap, fmt, rc::Rc};

use crate::parser::{AstNode, Function, Node, Op, Position};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Array(Rc<Vec<Value>>),
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
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
            Function(function) => {
                let raw = Rc::into_raw(function.clone());
                write!(f, "function: {:?}", raw)
            }
        }
    }
}

struct Scope {
    values: HashMap<Rc<String>, Value>,
}

impl Scope {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

macro_rules! runtime_error {
    ($position:expr, $error:expr) => {
        Err(format!(
            "Runtime error at line: {} column: {}\n - {}",
            $position.line, $position.column, $error
        ))
    };
}

pub struct Runtime {
    _global: Scope,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            _global: Scope::new(),
        }
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> Result<Value, String> {
        self.evaluate_block(ast, &mut Scope::new())
    }

    fn evaluate_block(&mut self, block: &Vec<AstNode>, scope: &mut Scope) -> Result<Value, String> {
        let mut result = Value::Empty;
        for node in block.iter() {
            result = self.evaluate(node, scope)?;
        }
        Ok(result)
    }

    fn evaluate(&mut self, node: &AstNode, scope: &mut Scope) -> Result<Value, String> {
        use Value::*;

        match &node.node {
            Node::Bool(b) => Ok(Bool(*b)),
            Node::Number(n) => Ok(Number(*n)),
            Node::Str(s) => Ok(StrLiteral(s.clone())),
            Node::Array(elements) => {
                let values: Result<Vec<_>, _> = elements
                    .iter()
                    .map(|node| self.evaluate(node, scope))
                    .collect();
                Ok(Array(Rc::new(values?)))
            }
            Node::Index { id, expression } => {
                let index = self.evaluate(expression, scope)?;
                match index {
                    Number(i) => {
                        let i = i as usize;
                        match &scope.values.get(id) {
                            Some(range) => match range {
                                Array(elements) => {
                                    if i < elements.len() {
                                        Ok(elements[i].clone())
                                    } else {
                                        runtime_error!(
                                        node.position,
                                        format!("Index out of bounds: '{}' has a length of {} but the index is {}",
                                                id, elements.len(), i))
                                    }
                                }
                                _ => runtime_error!(
                                    node.position,
                                    format!("Unable to index {}", range)
                                ),
                            },
                            None => {
                                runtime_error!(node.position, format!("Value not found: '{}'", id))
                            }
                        }
                    }
                    _ => runtime_error!(
                        node.position,
                        format!(
                            "Indexing is only supported with number values (got index value of {})",
                            index
                        )
                    ),
                }
            }
            Node::Id(id) => self.get_value(id, scope, node.position),
            Node::Block(block) => self.evaluate_block(&block, scope),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => {
                let maybe_function_or_error = scope.values.get(function).map(|f| match f {
                    Function(f) => Ok(f.clone()),
                    unexpected => runtime_error!(
                        node.position,
                        format!(
                            "Expected function for value {}, found {}",
                            function, unexpected
                        )
                    ),
                });
                if let Some(f) = maybe_function_or_error {
                    let f = f?;
                    let arg_count = f.args.len();
                    if args.len() != arg_count {
                        return runtime_error!(
                            node.position,
                            format!(
                                "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                                function,
                                arg_count,
                                args.len(),
                                f.args
                            )
                        );
                    }

                    let mut child_scope = Scope::new();

                    for (name, arg) in f.args.iter().zip(args.iter()) {
                        let arg_value = self.evaluate(arg, scope)?;
                        child_scope.values.insert(name.clone(), arg_value);
                    }

                    return self.evaluate_block(&f.body, &mut child_scope);
                }

                let mut arg_values = args.iter().map(|arg| self.evaluate(arg, scope));
                // Builtins, TODO std lib
                match function.as_str() {
                    "assert" => {
                        for value in arg_values {
                            match value? {
                                Bool(b) => {
                                    if !b {
                                        return runtime_error!(
                                            node.position,
                                            format!("Assertion failed")
                                        );
                                    }
                                }
                                _ => {
                                    return runtime_error!(
                                        node.position,
                                        format!("assert only expects booleans as arguments")
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
                                return runtime_error!(
                                    node.position,
                                    "Missing array as first argument for push"
                                );
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
                                    node.position,
                                    format!(
                                        "push is only supported for arrays, found {}",
                                        unexpected
                                    )
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
                    _ => runtime_error!(
                        node.position,
                        format!("Unexpected function name: {}", function.as_str())
                    ),
                }
            }
            Node::Assign { lhs, rhs } => {
                let value = self.evaluate(rhs, scope)?;
                self.set_value(lhs.clone(), value.clone(), scope);
                Ok(value)
            }
            Node::BinaryOp { lhs, op, rhs } => {
                let a = self.evaluate(lhs, scope)?;
                let b = self.evaluate(rhs, scope)?;
                macro_rules! binary_op_error {
                    ($op:ident, $a:ident, $b:ident) => {
                        runtime_error!(
                            node.position,
                            format!(
                                "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
                                op, a, b
                            )
                        )
                    };
                };
                match (&a, &b) {
                    (Number(a), Number(b)) => match op {
                        Op::Add => Ok(Number(a + b)),
                        Op::Subtract => Ok(Number(a - b)),
                        Op::Multiply => Ok(Number(a * b)),
                        Op::Divide => Ok(Number(a / b)),
                        Op::Equal => Ok(Bool(a == b)),
                        Op::NotEqual => Ok(Bool(a != b)),
                        Op::LessThan => Ok(Bool(a < b)),
                        Op::LessThanOrEqual => Ok(Bool(a <= b)),
                        Op::GreaterThan => Ok(Bool(a > b)),
                        Op::GreaterThanOrEqual => Ok(Bool(a >= b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    (Bool(a), Bool(b)) => match op {
                        Op::Equal => Ok(Bool(a == b)),
                        Op::NotEqual => Ok(Bool(a != b)),
                        Op::And => Ok(Bool(*a && *b)),
                        Op::Or => Ok(Bool(*a || *b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    _ => binary_op_error!(op, a, b),
                }
            }
        }
    }

    fn get_value(
        &self,
        id: &Rc<String>,
        scope: &Scope,
        position: Position,
    ) -> Result<Value, String> {
        scope.values.get(id).map_or_else(
            || runtime_error!(position, format!("Value not found: '{}'", id)),
            |v| Ok(v.clone()),
        )
    }
}
