use pest::{error::Error, prec_climber::PrecClimber, Parser, Span};
use std::{fmt, rc::Rc};

#[derive(Parser)]
#[grammar = "song.pest"]
struct PestParser;

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

pub type Id = Rc<String>;

#[derive(Clone, Debug)]
pub struct LookupId(pub Vec<Id>);

impl fmt::Display for LookupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for id in self.0.iter() {
            if !first {
                write!(f, ".")?;
            }
            write!(f, "{}", id)?;
            first = false;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Id(LookupId),
    Bool(bool),
    Number(f64),
    Str(Rc<String>),
    List(Vec<AstNode>),
    Range {
        min: Box<AstNode>,
        max: Box<AstNode>,
        inclusive: bool,
    },
    Map(Vec<(Id, AstNode)>),
    Block(Vec<AstNode>),
    Function(Rc<Function>),
    Call {
        function: LookupId,
        args: Vec<AstNode>,
    },
    Index {
        id: LookupId,
        expression: Box<AstNode>,
    },
    Assign {
        id: Id,
        expression: Box<AstNode>,
    },
    MultiAssign {
        ids: Vec<Id>,
        expressions: Vec<AstNode>,
    },
    Op {
        op: AstOp,
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
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
    pub args: Vec<Id>,
    pub body: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub struct AstFor {
    pub args: Vec<Id>,
    pub ranges: Vec<AstNode>,
    pub condition: Option<Box<AstNode>>,
    pub body: Box<AstNode>,
}

#[derive(Clone, Debug)]
pub enum AstOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    And,
    Or,
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub struct SongParser {
    climber: PrecClimber<Rule>,
}

impl SongParser {
    pub fn new() -> Self {
        use pest::prec_climber::{Assoc::*, Operator};
        use Rule::*;

        Self {
            climber: PrecClimber::new(vec![
                Operator::new(add, Left) | Operator::new(subtract, Left),
                Operator::new(multiply, Left)
                    | Operator::new(divide, Left)
                    | Operator::new(modulo, Left),
                Operator::new(and, Left) | Operator::new(or, Left),
                Operator::new(equal, Left) | Operator::new(not_equal, Left),
                Operator::new(greater, Left)
                    | Operator::new(greater_or_equal, Left)
                    | Operator::new(less, Left)
                    | Operator::new(less_or_equal, Left),
            ]),
        }
    }

    pub fn parse(&self, source: &str) -> Result<Vec<AstNode>, Error<Rule>> {
        let parsed = PestParser::parse(Rule::program, source)?;
        // dbg!(&parsed);

        let mut ast = vec![];
        for pair in parsed {
            match pair.as_rule() {
                Rule::block => {
                    ast.push(self.build_ast(pair));
                }
                _ => {}
            }
        }

        Ok(ast)
    }

    fn build_ast(&self, pair: pest::iterators::Pair<Rule>) -> AstNode {
        // dbg!(&pair);
        use pest::iterators::Pair;
        use Node::*;

        macro_rules! next_as_boxed_ast {
            ($inner:expr) => {
                Box::new(self.build_ast($inner.next().unwrap()))
            };
        }

        macro_rules! next_as_rc_string {
            ($inner:expr) => {
                Rc::new($inner.next().unwrap().as_str().to_string())
            };
        }

        macro_rules! next_as_lookup_id {
            ($inner:expr) => {
                LookupId(
                    $inner
                        .next()
                        .unwrap()
                        .into_inner()
                        .map(|pair| Rc::new(pair.as_str().to_string()))
                        .collect::<Vec<_>>(),
                )
            };
        }

        let span = pair.as_span();
        match pair.as_rule() {
            Rule::next_expression => self.build_ast(pair.into_inner().next().unwrap()),
            Rule::block | Rule::child_block => {
                let inner = pair.into_inner();
                let block: Vec<AstNode> = inner.map(|pair| self.build_ast(pair)).collect();
                (AstNode::new(span, Block(block)))
            }
            Rule::expressions => {
                let inner = pair.into_inner();
                let expressions = inner.map(|pair| self.build_ast(pair)).collect::<Vec<_>>();

                if expressions.len() == 1 {
                    expressions.first().unwrap().clone()
                } else {
                    AstNode::new(span, Node::List(expressions))
                }
            }
            Rule::boolean => (AstNode::new(span, Bool(pair.as_str().parse().unwrap()))),
            Rule::number => (AstNode::new(span, Node::Number(pair.as_str().parse().unwrap()))),
            Rule::string => {
                let mut inner = pair.into_inner();
                (AstNode::new(span, Node::Str(next_as_rc_string!(inner))))
            }
            Rule::list => {
                let inner = pair.into_inner();
                let elements: Vec<AstNode> = inner.map(|pair| self.build_ast(pair)).collect();
                (AstNode::new(span, Node::List(elements)))
            }
            Rule::range => {
                let mut inner = pair.into_inner();

                let min = next_as_boxed_ast!(inner);
                let inclusive = inner.next().unwrap().as_str() == "..=";
                let max = next_as_boxed_ast!(inner);

                (AstNode::new(
                    span,
                    Node::Range {
                        min,
                        inclusive,
                        max,
                    },
                ))
            }
            Rule::map | Rule::map_inline => {
                let entries = pair
                    .into_inner()
                    .map(|pair| {
                        let mut inner = pair.into_inner();
                        let id = next_as_rc_string!(inner);
                        let value = self.build_ast(inner.next().unwrap());
                        (id, value)
                    })
                    .collect::<Vec<_>>();
                (AstNode::new(span, Node::Map(entries)))
            }
            Rule::index => {
                let mut inner = pair.into_inner();
                let id = next_as_lookup_id!(inner);
                let expression = next_as_boxed_ast!(inner);
                (AstNode::new(span, Node::Index { id, expression }))
            }
            Rule::id => {
                let id = LookupId(
                    pair.into_inner()
                        .map(|pair| Rc::new(pair.as_str().to_string()))
                        .collect::<Vec<_>>(),
                );
                AstNode::new(span, Node::Id(id))
            }
            Rule::function_block | Rule::function_inline => {
                let mut inner = pair.into_inner();
                let mut capture = inner.next().unwrap().into_inner();
                let args = capture
                    .by_ref()
                    .map(|pair| Rc::new(pair.as_str().to_string()))
                    .collect::<Vec<_>>();
                // collect function body
                let body: Vec<AstNode> = inner.map(|pair| self.build_ast(pair)).collect();
                (AstNode::new(span, Node::Function(Rc::new(self::Function { args, body }))))
            }
            Rule::call_with_parens | Rule::call_single_arg => {
                let mut inner = pair.into_inner();
                let function = next_as_lookup_id!(inner);
                let args = if inner.peek().unwrap().as_rule() == Rule::call_args {
                    inner
                        .next()
                        .unwrap()
                        .into_inner()
                        .map(|pair| self.build_ast(pair))
                        .collect::<Vec<_>>()
                } else {
                    vec![self.build_ast(inner.next().unwrap())]
                };
                (AstNode::new(span, Node::Call { function, args }))
            }
            Rule::single_assignment => {
                let mut inner = pair.into_inner();
                let id = next_as_rc_string!(inner.next().unwrap().into_inner());
                let expression = next_as_boxed_ast!(inner);
                (AstNode::new(span, Node::Assign { id, expression }))
            }
            Rule::multiple_assignment => {
                let mut inner = pair.into_inner();
                let ids = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| next_as_rc_string!(pair.into_inner()))
                    .collect::<Vec<_>>();
                let expressions = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(pair))
                    .collect::<Vec<_>>();
                (AstNode::new(span, Node::MultiAssign { ids, expressions }))
            }
            Rule::operation => {
                // dbg!(&pair);
                self.climber.climb(
                    pair.into_inner(),
                    |pair: Pair<Rule>| self.build_ast(pair),
                    |lhs: AstNode, op: Pair<Rule>, rhs: AstNode| {
                        let span = op.as_span();
                        let lhs = Box::new(lhs);
                        let rhs = Box::new(rhs);
                        use AstOp::*;
                        macro_rules! make_ast_op {
                            ($op:expr) => {
                                AstNode::new(span, Node::Op { op: $op, lhs, rhs })
                            };
                        };
                        match op.as_rule() {
                            Rule::add => make_ast_op!(Add),
                            Rule::subtract => make_ast_op!(Subtract),
                            Rule::multiply => make_ast_op!(Multiply),
                            Rule::divide => make_ast_op!(Divide),
                            Rule::modulo => make_ast_op!(Modulo),
                            Rule::equal => make_ast_op!(Equal),
                            Rule::not_equal => make_ast_op!(NotEqual),
                            Rule::greater => make_ast_op!(Greater),
                            Rule::greater_or_equal => make_ast_op!(GreaterOrEqual),
                            Rule::less => make_ast_op!(Less),
                            Rule::less_or_equal => make_ast_op!(LessOrEqual),
                            Rule::and => make_ast_op!(And),
                            Rule::or => make_ast_op!(Or),
                            unexpected => {
                                let error = format!("Unexpected operator: {:?}", unexpected);
                                unreachable!(error)
                            }
                        }
                    },
                )
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

                (AstNode::new(
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

                (AstNode::new(
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
                let args = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| Rc::new(pair.as_str().to_string()))
                    .collect::<Vec<_>>();
                inner.next(); // in
                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(pair))
                    .collect::<Vec<_>>();
                let condition = if inner.peek().unwrap().as_rule() == Rule::if_keyword {
                    inner.next();
                    Some(next_as_boxed_ast!(inner))
                } else {
                    None
                };
                let body = next_as_boxed_ast!(inner);
                (AstNode::new(
                    span,
                    Node::For(Rc::new(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    })),
                ))
            }
            Rule::for_inline => {
                let mut inner = pair.into_inner();
                let body = next_as_boxed_ast!(inner);
                inner.next(); // for
                let args = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| Rc::new(pair.as_str().to_string()))
                    .collect::<Vec<_>>();
                inner.next(); // in
                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(pair))
                    .collect::<Vec<_>>();
                let condition = if inner.next().is_some() {
                    // if
                    Some(next_as_boxed_ast!(inner))
                } else {
                    None
                };
                (AstNode::new(
                    span,
                    Node::For(Rc::new(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    })),
                ))
            }
            unexpected => unreachable!("Unexpected expression: {:?}", unexpected),
        }
    }
}
