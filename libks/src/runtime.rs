use std::{collections::HashMap, rc::Rc};

use crate::parser::{AstNode, Function, Node, Op};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
}

pub struct Runtime {
    values: HashMap<String, Value>, // TODO Rc string
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> Result<Value, String> {
        self.evaluate_block(ast)
    }

    pub fn evaluate_block(&mut self, block: &Vec<AstNode>) -> Result<Value, String> {
        let mut result = Value::Empty;
        for node in block.iter() {
            result = self.evaluate(node)?;
        }
        Ok(result)
    }

    pub fn evaluate(&mut self, node: &AstNode) -> Result<Value, String> {
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
            Node::Ident(ident) => self.values.get(ident).map_or_else(
                || runtime_error!(node.position, format!("Variable not found: '{}'", ident)),
                |v| Ok(v.clone()),
            ),
            Node::Block(block) => self.evaluate_block(&block),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => {
                let f = self.values.get(function).map(|f| match f {
                    Function(f) => Ok(f.clone()),
                    unexpected => runtime_error!(
                        node.position,
                        format!(
                            "Expected function for value {}, found {:?}",
                            function, unexpected
                        )
                    ),
                });
                if let Some(f) = f {
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

                    for (name, arg) in f.args.iter().zip(args.iter()) {
                        let arg_value = self.evaluate(arg)?;
                        self.values.insert(name.clone(), arg_value);
                    }

                    return self.evaluate_block(&f.body);
                }

                let arg_values = args.iter().map(|arg| self.evaluate(arg));
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
                let value = self.evaluate(rhs)?;
                self.values.insert(lhs.clone(), value);
                Ok(Empty)
            }
            Node::BinaryOp { lhs, op, rhs } => {
                let a = self.evaluate(lhs)?;
                let b = self.evaluate(rhs)?;
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
