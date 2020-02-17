use pest::{error::Error, Parser, Span};
use std::rc::Rc;

#[derive(Parser)]
#[grammar = "ks.pest"]
struct KsParser;

#[derive(Clone, Debug)]
pub struct AstNode {
    pub position: Position,
    pub node: Node,
}

impl AstNode {
    pub fn new(span: Span, node: Node) -> Self {
        let line_col = span.start_pos().line_col();
        let position = Position {
            line: line_col.0,
            column: line_col.1,
        };
        Self { position, node }
    }
}

#[derive(Clone, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub enum Node {
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
pub struct Function {
    pub args: Vec<String>,
    pub body: Vec<AstNode>,
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
    // dbg!(&parsed);

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
    use Node::*;
    match pair.as_rule() {
        Rule::expression => build_ast_from_expression(pair.into_inner().next().unwrap()),
        Rule::boolean => AstNode::new(pair.as_span(), Bool(pair.as_str().parse().unwrap())),
        Rule::number => AstNode::new(pair.as_span(), Node::Number(pair.as_str().parse().unwrap())),
        Rule::string => AstNode::new(
            pair.as_span(),
            Node::Str(Rc::new(
                pair.into_inner().next().unwrap().as_str().to_string(),
            )),
        ),
        Rule::ident => AstNode::new(pair.as_span(), Node::Ident(pair.as_str().to_string())),
        Rule::function => {
            let span = pair.as_span();
            let mut inner = pair.into_inner();
            // collect any arguments before the function operator
            let args: Vec<String> = inner
                .by_ref()
                .take_while(|pair| pair.as_str() != "->")
                .map(|pair| pair.as_str().to_string())
                .collect();
            // collect function body
            let body: Vec<AstNode> = inner.map(|pair| build_ast_from_expression(pair)).collect();
            AstNode::new(span, Node::Function(Rc::new(self::Function { args, body })))
        }
        Rule::call => {
            let span = pair.as_span();
            let mut inner = pair.into_inner();
            let function: String = inner.next().unwrap().as_str().to_string();
            let args: Vec<AstNode> = inner.map(|pair| build_ast_from_expression(pair)).collect();
            AstNode::new(span, Node::Call { function, args })
        }
        Rule::assignment => {
            let span = pair.as_span();
            let mut inner = pair.into_inner();
            let lhs = inner.next().unwrap().as_str().to_string();
            let rhs = Box::new(build_ast_from_expression(inner.next().unwrap()));
            AstNode::new(span, Node::Assign { lhs, rhs })
        }
        Rule::binary_op => {
            let span = pair.as_span();
            let mut inner = pair.into_inner();
            let lhs = Box::new(build_ast_from_expression(inner.next().unwrap()));
            let op = match inner.next().unwrap().as_str() {
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
            let rhs = Box::new(build_ast_from_expression(inner.next().unwrap()));
            AstNode::new(span, Node::BinaryOp { lhs, op, rhs })
        }
        unexpected => unreachable!("Unexpected expression: {:?}", unexpected),
    }
}
