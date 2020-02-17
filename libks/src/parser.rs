use pest::{error::Error, Parser};
use std::rc::Rc;

#[derive(Parser)]
#[grammar = "ks.pest"]
struct KsParser;

#[derive(Clone, Debug)]
pub struct Function {
    pub args: Vec<String>,
    pub body: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub enum AstNode {
    Bool(bool),
    Number(f64),
    Str(Rc<String>),
    Ident(String),
    Function(Rc<Function>),
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

#[derive(Clone, Debug)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
}

pub fn parse(source: &str) -> Result<Vec<AstNode>, Error<Rule>> {
    let parsed = KsParser::parse(Rule::program, source)?;

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
    // dbg!(&pair);
    match pair.as_rule() {
        Rule::expression => build_ast_from_expression(pair.into_inner().next().unwrap()),
        Rule::boolean => AstNode::Bool(pair.as_str().parse().unwrap()),
        Rule::number => AstNode::Number(pair.as_str().parse().unwrap()),
        Rule::string => AstNode::Str(Rc::new(
            pair.into_inner().next().unwrap().as_str().to_string(),
        )),
        Rule::ident => AstNode::Ident(pair.as_str().to_string()),
        Rule::function => {
            let mut pair = pair.into_inner();
            let args: Vec<String> = pair
                .by_ref()
                .take_while(|pair| pair.as_str() != "->")
                .map(|pair| pair.as_str().to_string())
                .collect();
            let body: Vec<AstNode> = pair.map(|pair| build_ast_from_expression(pair)).collect();
            AstNode::Function(Rc::new(Function { args, body }))
        }
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
                "==" => Op::Equal,
                "!=" => Op::NotEqual,
                "<" => Op::LessThan,
                "<=" => Op::LessThanOrEqual,
                ">" => Op::GreaterThan,
                ">=" => Op::GreaterThanOrEqual,
                "and" => Op::And,
                "or" => Op::Or,
                unexpected => {
                    let error = format!("Unexpected binary operator: {}", unexpected);
                    unreachable!(error)
                }
            };
            let rhs = Box::new(build_ast_from_expression(pair.next().unwrap()));
            AstNode::BinaryOp { lhs, op, rhs }
        }
        unexpected => panic!("Unexpected expression: {:?}", unexpected),
    }
}
