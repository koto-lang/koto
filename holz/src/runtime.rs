use std::{collections::HashMap, rc::Rc};

use crate::parser::{AstNode, Function, Node, Op};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Array(Vec<Value>),
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
}

struct Scope {
    values: HashMap<Rc<String>, Value>, // TODO Rc string
}

impl Scope {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
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

        macro_rules! runtime_error {
            ($position:expr, $error:expr) => {
                Err(format!(
                    "Runtime error at line: {} column: {} - {}",
                    $position.line, $position.column, $error
                ))
            };
        };

        match &node.node {
            Node::Bool(b) => Ok(Bool(*b)),
            Node::Number(n) => Ok(Number(*n)),
            Node::Str(s) => Ok(StrLiteral(s.clone())),
            Node::Array(elements) => {
                let values: Result<Vec<_>, _> = elements
                    .iter()
                    .map(|node| self.evaluate(node, scope))
                    .collect();
                Ok(Array(values?))
            }
            Node::Ident(ident) => scope.values.get(ident).map_or_else(
                || runtime_error!(node.position, format!("Variable not found: '{}'", ident)),
                |v| Ok(v.clone()),
            ),
            Node::Block(block) => self.evaluate_block(&block, scope),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => {
                let maybe_function_or_error = scope.values.get(function).map(|f| match f {
                    Function(f) => Ok(f.clone()),
                    unexpected => runtime_error!(
                        node.position,
                        format!(
                            "Expected function for value {}, found {:?}",
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
                                "Incorrect argument count while calling '{}': expected {}, found {}",
                                function,
                                arg_count,
                                args.len()
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

                let arg_values = args.iter().map(|arg| self.evaluate(arg, scope));
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
                    "print" => {
                        for value in arg_values {
                            match value? {
                                Empty => print!("() "),
                                Bool(s) => print!("{} ", s),
                                Number(n) => print!("{} ", n),
                                StrLiteral(s) => print!("{} ", s),
                                Array(a) => print!("{:?} ", a),
                                Function(_) => {
                                    return runtime_error!(
                                        node.position,
                                        "print doesn't accept functions as arguments".to_string()
                                    )
                                }
                            }
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
                scope.values.insert(lhs.clone(), value.clone());
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
                                "Unable to perform operation {:?} with lhs: '{:?}' and rhs: '{:?}'",
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
}
