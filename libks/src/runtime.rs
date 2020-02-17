use std::collections::HashMap;

use crate::parser::{AstNode, Op};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
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
                                Str(s) => print!("{} ", s),
                                Number(n) => print!("{} ", n),
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
                match (&a, &b) {
                    (Number(a), Number(b)) => Ok(Number(match op {
                        Op::Add => a + b,
                        Op::Subtract => a - b,
                        Op::Multiply => a * b,
                        Op::Divide => a / b,
                    })),
                    _ => Err(format!(
                        "Unable to perform binary operation with lhs: '{:?}' and rhs: '{:?}'",
                        a, b
                    )),
                }
            }
        }
    }
}
