use std::collections::HashMap;

use crate::parser::{AstNode, Op};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Str(String),
}

pub struct Runtime {
    values: HashMap<String, Value>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> Result<(), String> {
        for node in ast.iter() {
            self.evaluate(node)?;
        }
        Ok(())
    }

    pub fn evaluate(&mut self, node: &AstNode) -> Result<Value, String> {
        use Value::*;

        match node {
            AstNode::Bool(b) => Ok(Bool(*b)),
            AstNode::Number(n) => Ok(Number(*n)),
            AstNode::Str(s) => Ok(Str(s.clone())),
            AstNode::Ident(ident) => self.values.get(ident).map_or_else(
                || Err(format!("Identifier not found: '{}'", ident)),
                |v| Ok(v.clone()),
            ),
            AstNode::Call { function, args } => {
                let values = args.iter().map(|arg| self.evaluate(arg));
                match function.as_str() {
                    "print" => {
                        for value in values {
                            match value? {
                                Empty => print!("() "),
                                Bool(s) => print!("{} ", s),
                                Number(n) => print!("{} ", n),
                                Str(s) => print!("{} ", s),
                            }
                        }
                        println!();
                        Ok(Empty)
                    }
                    _ => unimplemented!(),
                }
            }
            AstNode::Assign { lhs, rhs } => {
                let value = self.evaluate(rhs)?;
                self.values.insert(lhs.clone(), value);
                Ok(Empty)
            }
            AstNode::BinaryOp { lhs, op, rhs } => {
                let a = self.evaluate(lhs)?;
                let b = self.evaluate(rhs)?;
                macro_rules! binary_op_error {
                    ($op:ident, $a:ident, $b:ident) => {
                        Err(format!(
                            "Unable to perform operation {:?} with lhs: '{:?}' and rhs: '{:?}'",
                            op, a, b
                        ))
                    };
                };
                match (&a, &b) {
                    (Number(a), Number(b)) => match op {
                        Op::Add => Ok(Number(a + b)),
                        Op::Subtract => Ok(Number(a - b)),
                        Op::Multiply => Ok(Number(a * b)),
                        Op::Divide => Ok(Number(a / b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    (Bool(a), Bool(b)) => match op {
                        Op::Equal => Ok(Bool(a == b)),
                        Op::NotEqual => Ok(Bool(a != b)),
                        Op::LessThan => Ok(Bool(a < b)),
                        Op::LessThanOrEqual => Ok(Bool(a <= b)),
                        Op::GreaterThan => Ok(Bool(a > b)),
                        Op::GreaterThanOrEqual => Ok(Bool(a >= b)),
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
