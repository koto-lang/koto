#[macro_use]
extern crate pest_derive;
use pest::{error::Error, Parser};
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "ks.pest"]
struct KsParser;

#[derive(Debug)]
enum AstNode {
    Number(f64),
    Str(String),
    Ident(String),
    Call {
        function: String,
        args: Vec<AstNode>,
    },
    Assign {
        lhs: String,
        rhs: Box<AstNode>,
    },
    BinaryOp {
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
        op: Op,
    },
}

#[derive(Debug)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
}

fn parse(source: &str) -> Result<Vec<AstNode>, Error<Rule>> {
    let parsed = KsParser::parse(Rule::program, source)?;

    println!("{}", source);
    println!("{}", parsed);

    let mut ast = vec![];
    for pair in parsed {
        match pair.as_rule() {
            Rule::expression => {
                ast.push(build_ast_from_expression(pair));
            }
            _ => {}
        }
    }

    Ok(ast)
}

fn build_ast_from_expression(pair: pest::iterators::Pair<Rule>) -> AstNode {
    match pair.as_rule() {
        Rule::expression => build_ast_from_expression(pair.into_inner().next().unwrap()),
        Rule::number => AstNode::Number(pair.as_str().parse().unwrap()),
        Rule::string => AstNode::Str(pair.into_inner().next().unwrap().as_str().to_string()),
        Rule::ident => AstNode::Ident(pair.as_str().into()),
        Rule::call => {
            let mut pair = pair.into_inner();
            let function: String = pair.next().unwrap().as_str().to_string();
            let args: Vec<AstNode> = pair.map(|pair| build_ast_from_expression(pair)).collect();
            AstNode::Call { function, args }
        }
        Rule::assignment => {
            let mut pair = pair.into_inner();
            let lhs = pair.next().unwrap().as_str().to_string();
            let rhs = Box::new(build_ast_from_expression(pair.next().unwrap()));
            AstNode::Assign { lhs, rhs }
        }
        Rule::binary_op => {
            let mut pair = pair.into_inner();
            let lhs = Box::new(build_ast_from_expression(pair.next().unwrap()));
            let op = match pair.next().unwrap().as_str() {
                "+" => Op::Add,
                "-" => Op::Subtract,
                "*" => Op::Multiply,
                "/" => Op::Divide,
                _ => unreachable!(),
            };
            let rhs = Box::new(build_ast_from_expression(pair.next().unwrap()));
            AstNode::BinaryOp { lhs, op, rhs }
        }
        unexpected => panic!("Unexpected expression: {:?}", unexpected),
    }
}

#[derive(Clone, Debug)]
enum Value {
    Empty,
    Number(f64),
    Str(String),
}

struct Runtime {
    values: HashMap<String, Value>,
}

impl Runtime {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    fn run(&mut self, ast: &Vec<AstNode>) -> Result<(), String> {
        for node in ast.iter() {
            self.evaluate(node)?;
        }
        Ok(())
    }

    fn evaluate(&mut self, node: &AstNode) -> Result<Value, String> {
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

fn main() {
    let script = r#"
        // Comment
        print("Hello, World!!!")
        print(42.0)
        a = 2
        b = a * 8 + 4
        print(b, 43.0, "Hiii")
    "#;

    match parse(script) {
        Ok(ast) => {
            println!("{:?}\n", ast);
            let mut runtime = Runtime::new();
            match runtime.run(&ast) {
                Ok(_) => {}
                Err(e) => println!("Error while running script:\n  {}", e),
            }
        }
        Err(e) => println!("Error while parsing source: {}", e),
    }
}
