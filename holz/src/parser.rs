use pest::{error::Error, Parser, Span};
use std::rc::Rc;

#[derive(Parser)]
#[grammar = "holz.pest"]
struct HolzParser;

#[derive(Clone, Debug)]
pub struct AstNode {
    pub node: Node,
    pub start_pos: Position,
    pub end_pos: Position,
}

impl AstNode {
    pub fn new(span: Span, node: Node) -> Self {
        let line_col = span.start_pos().line_col();
        let start_pos = Position {
            line: line_col.0,
            column: line_col.1,
        };
        let line_col = span.end_pos().line_col();
        let end_pos = Position {
            line: line_col.0,
            column: line_col.1,
        };
        Self {
            node,
            start_pos,
            end_pos,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Bool(bool),
    Number(f64),
    Str(Rc<String>),
    Array(Vec<AstNode>),
    Range {
        min: Box<AstNode>,
        max: Box<AstNode>,
        inclusive: bool,
    },
    Id(Rc<String>),
    Block(Vec<AstNode>),
    Function(Rc<Function>),
    Call {
        function: Rc<String>,
        args: Vec<AstNode>,
    },
    Index {
        id: String,
        expression: Box<AstNode>,
    },
    Assign {
        id: Rc<String>,
        expression: Box<AstNode>,
    },
    BinaryOp {
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
        op: Op,
    },
    If {
        condition: Box<AstNode>,
        then_node: Box<AstNode>,
        else_node: Option<Box<AstNode>>,
    },
    For(Rc<AstFor>),
}

#[derive(Clone, Debug)]
pub struct Block {}

#[derive(Clone, Debug)]
pub struct Function {
    pub args: Vec<Rc<String>>,
    pub body: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub struct AstFor {
    pub arg: Rc<String>,
    pub range: Box<AstNode>,
    pub condition: Option<Box<AstNode>>,
    pub body: Box<AstNode>,
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

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub fn parse(source: &str) -> Result<Vec<AstNode>, Error<Rule>> {
    let parsed = HolzParser::parse(Rule::program, source)?;
    // dbg!(&parsed);

    let mut ast = vec![];
    for pair in parsed {
        match pair.as_rule() {
            Rule::block => {
                ast.push(build_ast_from_expression(pair).unwrap());
            }
            _ => {}
        }
    }

    Ok(ast)
}

fn build_ast_from_expression(pair: pest::iterators::Pair<Rule>) -> Option<AstNode> {
    // dbg!(&pair);
    use Node::*;

    macro_rules! next_as_boxed_ast {
        ($inner:expr) => {
            Box::new(build_ast_from_expression($inner.next().unwrap()).unwrap())
        };
    }

    macro_rules! next_as_rc_string {
        ($inner:expr) => {
            Rc::new($inner.next().unwrap().as_str().to_string())
        };
    }

    let span = pair.as_span();
    match pair.as_rule() {
        Rule::expression | Rule::next_expression | Rule::lhs_value | Rule::rhs_value => {
            build_ast_from_expression(pair.into_inner().next().unwrap())
        }
        Rule::block | Rule::child_block => {
            let inner = pair.into_inner();
            let block: Vec<AstNode> = inner
                .filter_map(|pair| build_ast_from_expression(pair))
                .collect();
            Some(AstNode::new(span, Block(block)))
        }
        Rule::boolean => Some(AstNode::new(span, Bool(pair.as_str().parse().unwrap()))),
        Rule::number => Some(AstNode::new(
            span,
            Node::Number(pair.as_str().parse().unwrap()),
        )),
        Rule::string => {
            let mut inner = pair.into_inner();
            Some(AstNode::new(span, Node::Str(next_as_rc_string!(inner))))
        }
        Rule::array => {
            let inner = pair.into_inner();
            let elements: Vec<AstNode> = inner
                .filter_map(|pair| build_ast_from_expression(pair))
                .collect();
            Some(AstNode::new(span, Node::Array(elements)))
        }
        Rule::range => {
            let mut inner = pair.into_inner();

            let min = next_as_boxed_ast!(inner);
            let inclusive = inner.next().unwrap().as_str() == "..=";
            let max = next_as_boxed_ast!(inner);

            Some(AstNode::new(
                span,
                Node::Range {
                    min,
                    inclusive,
                    max,
                },
            ))
        }
        Rule::index => {
            let mut inner = pair.into_inner();
            let id = inner.next().unwrap().as_str().to_string();
            let expression = next_as_boxed_ast!(inner);
            Some(AstNode::new(span, Node::Index { id, expression }))
        }
        Rule::id => Some(AstNode::new(
            span,
            Node::Id(Rc::new(pair.as_str().to_string())),
        )),
        Rule::function => {
            let mut inner = pair.into_inner();
            let mut capture = inner.next().unwrap().into_inner();
            let args: Vec<Rc<String>> = capture
                .by_ref()
                .take_while(|pair| pair.as_str() != "->")
                .map(|pair| Rc::new(pair.as_str().to_string()))
                .collect();
            // collect function body
            let body: Vec<AstNode> = inner
                .filter_map(|pair| build_ast_from_expression(pair))
                .collect();
            Some(AstNode::new(
                span,
                Node::Function(Rc::new(self::Function { args, body })),
            ))
        }
        Rule::call => {
            let mut inner = pair.into_inner();
            let function = next_as_rc_string!(inner);
            let args: Vec<AstNode> = inner
                .filter_map(|pair| build_ast_from_expression(pair))
                .collect();
            Some(AstNode::new(span, Node::Call { function, args }))
        }
        Rule::assignment => {
            let mut inner = pair.into_inner();
            let id = next_as_rc_string!(inner);
            let expression = next_as_boxed_ast!(inner);
            Some(AstNode::new(span, Node::Assign { id, expression }))
        }
        Rule::binary_op => {
            let mut inner = pair.into_inner();
            let lhs = next_as_boxed_ast!(inner);
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
            let rhs = next_as_boxed_ast!(inner);
            Some(AstNode::new(span, Node::BinaryOp { lhs, op, rhs }))
        }
        Rule::if_inline => {
            let mut inner = pair.into_inner();
            inner.next(); // if
            let condition = next_as_boxed_ast!(inner);
            inner.next(); // then
            let then_node = next_as_boxed_ast!(inner);
            let else_node = if inner.next().is_some() {
                Some(next_as_boxed_ast!(inner))
            } else {
                None
            };

            Some(AstNode::new(
                span,
                Node::If {
                    condition,
                    then_node,
                    else_node,
                },
            ))
        }
        Rule::if_block => {
            let mut inner = pair.into_inner();
            inner.next(); // if
            let condition = next_as_boxed_ast!(inner);
            let then_node = next_as_boxed_ast!(inner);
            let else_node = if inner.peek().is_some() {
                Some(next_as_boxed_ast!(inner))
            } else {
                None
            };

            Some(AstNode::new(
                span,
                Node::If {
                    condition,
                    then_node,
                    else_node,
                },
            ))
        }
        Rule::for_block => {
            let mut inner = pair.into_inner();
            inner.next(); // for
            let arg = next_as_rc_string!(inner);
            inner.next(); // in
            let range = next_as_boxed_ast!(inner);
            let condition = if inner.peek().unwrap().as_rule() == Rule::if_keyword {
                inner.next();
                Some(next_as_boxed_ast!(inner))
            } else {
                None
            };
            let body = next_as_boxed_ast!(inner);
            Some(AstNode::new(
                span,
                Node::For(Rc::new(AstFor {
                    arg,
                    range,
                    condition,
                    body,
                })),
            ))
        }
        Rule::for_inline => {
            let mut inner = pair.into_inner();
            let body = next_as_boxed_ast!(inner);
            inner.next(); // for
            let arg = next_as_rc_string!(inner);
            inner.next(); // in
            let range = next_as_boxed_ast!(inner);
            let condition = if inner.next().is_some() {
                // if
                Some(next_as_boxed_ast!(inner))
            } else {
                None
            };
            Some(AstNode::new(
                span,
                Node::For(Rc::new(AstFor {
                    arg,
                    range,
                    condition,
                    body,
                })),
            ))
        }
        unexpected => unreachable!("Unexpected expression: {:?}", unexpected),
    }
}
