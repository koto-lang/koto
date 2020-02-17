
use pest::{error::Error, Parser};

#[derive(Parser)]
#[grammar = "ks.pest"]
struct KsParser;

#[derive(Debug)]
pub enum AstNode {
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
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
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

