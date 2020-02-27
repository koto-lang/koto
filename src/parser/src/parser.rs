use crate::{prec_climber::PrecClimber, vec4};
use pest::{error::Error, Parser, Span};
use std::{fmt, rc::Rc};

use koto_grammar::Rule;

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

pub type Ast = Vec<AstNode>;

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
    Vec4(vec4::Vec4),
    Str(Rc<String>),
    List(Vec<AstNode>),
    Range {
        min: Box<AstNode>,
        max: Box<AstNode>,
        inclusive: bool,
    },
    Map(Vec<(Id, AstNode)>),
    Block(Vec<AstNode>),
    Expressions(Vec<AstNode>),
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
        global: bool,
    },
    MultiAssign {
        ids: Vec<Id>,
        expressions: Vec<AstNode>,
        global: bool,
    },
    Op {
        op: AstOp,
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
    },
    If {
        condition: Box<AstNode>,
        then_node: Box<AstNode>,
        else_if_condition: Option<Box<AstNode>>,
        else_if_node: Option<Box<AstNode>>,
        else_node: Option<Box<AstNode>>,
    },
    For(Rc<AstFor>),
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Node::*;
        match self {
            Id(lookup) => write!(f, "Id: {}", lookup),

            Bool(b) => write!(f, "Bool: {}", b),
            Number(n) => write!(f, "Number: {}", n),
            Vec4(v) => write!(f, "Vec4: {:?}", v),
            Str(s) => write!(f, "Str: {}", s),
            List(l) => write!(
                f,
                "List with {} {}",
                l.len(),
                if l.len() == 1 { "entry" } else { "entries" }
            ),
            Range { inclusive, .. } => {
                write!(f, "Range: {}", if *inclusive { "..=" } else { ".." },)
            }
            Map(m) => write!(
                f,
                "Map with {} {}",
                m.len(),
                if m.len() == 1 { "entry" } else { "entries" }
            ),
            Block(b) => write!(
                f,
                "Block with {} expression{}",
                b.len(),
                if b.len() == 1 { "" } else { "s" }
            ),
            Expressions(e) => write!(
                f,
                "Expressions with {} expression{}",
                e.len(),
                if e.len() == 1 { "" } else { "s" }
            ),
            Function(_) => write!(f, "Function"),
            Call { function, .. } => write!(f, "Call: {}", function),
            Index { id, .. } => write!(f, "Index: {}", id),
            Assign { id, global, .. } => write!(f, "Assign: id: {} - global: {}", id, global),
            MultiAssign { ids, global, .. } => {
                write!(f, "MultiAssign: ids: {:?} - global: {}", ids, global)
            }
            Op { op, .. } => write!(f, "Op: {:?}", op),
            If { .. } => write!(f, "If"),
            For(_) => write!(f, "For"),
        }
    }
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

pub struct KotoParser {
    climber: PrecClimber<Rule>,
}

impl KotoParser {
    pub fn new() -> Self {
        use crate::prec_climber::{Assoc::*, Operator};
        use Rule::*;

        Self {
            climber: PrecClimber::new(
                vec![
                    Operator::new(and, Left) | Operator::new(or, Left),
                    Operator::new(equal, Left) | Operator::new(not_equal, Left),
                    Operator::new(greater, Left)
                        | Operator::new(greater_or_equal, Left)
                        | Operator::new(less, Left)
                        | Operator::new(less_or_equal, Left),
                    Operator::new(add, Left) | Operator::new(subtract, Left),
                    Operator::new(multiply, Left)
                        | Operator::new(divide, Left)
                        | Operator::new(modulo, Left),
                ],
                vec![empty_line],
            ),
        }
    }

    pub fn parse(&self, source: &str) -> Result<Ast, Error<Rule>> {
        let parsed = koto_grammar::KotoParser::parse(Rule::program, source)?;

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
                AstNode::new(span, Block(block))
            }
            Rule::expressions | Rule::value_terms => {
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
                AstNode::new(span, Node::Str(next_as_rc_string!(inner)))
            }
            Rule::list => {
                let inner = pair.into_inner();
                let elements: Vec<AstNode> = inner.map(|pair| self.build_ast(pair)).collect();
                AstNode::new(span, Node::List(elements))
            }
            Rule::range => {
                let mut inner = pair.into_inner();

                let min = next_as_boxed_ast!(inner);
                let inclusive = inner.next().unwrap().as_str() == "..=";
                let max = next_as_boxed_ast!(inner);

                AstNode::new(
                    span,
                    Node::Range {
                        min,
                        inclusive,
                        max,
                    },
                )
            }
            Rule::map | Rule::map_value | Rule::map_inline => {
                // dbg!(&pair);
                let inner = if pair.as_rule() == Rule::map_value {
                    pair.into_inner().next().unwrap().into_inner()
                // pair.into_inner()
                } else {
                    pair.into_inner()
                };
                let entries = inner
                    .map(|pair| {
                        let mut inner = pair.into_inner();
                        let id = next_as_rc_string!(inner);
                        let value = self.build_ast(inner.next().unwrap());
                        (id, value)
                    })
                    .collect::<Vec<_>>();
                AstNode::new(span, Node::Map(entries))
            }
            Rule::index => {
                let mut inner = pair.into_inner();
                let id = next_as_lookup_id!(inner);
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::Index { id, expression })
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
                AstNode::new(span, Node::Function(Rc::new(self::Function { args, body })))
            }
            Rule::call_with_parens | Rule::call_no_parens => {
                let mut inner = pair.into_inner();
                let function = next_as_lookup_id!(inner);
                let args = match inner.peek().unwrap().as_rule() {
                    Rule::call_args | Rule::operations => inner
                        .next()
                        .unwrap()
                        .into_inner()
                        .map(|pair| self.build_ast(pair))
                        .collect::<Vec<_>>(),
                    _ => vec![self.build_ast(inner.next().unwrap())],
                };
                AstNode::new(span, Node::Call { function, args })
            }
            Rule::single_assignment => {
                let mut inner = pair.into_inner();
                let global = inner.peek().unwrap().as_rule() == Rule::global_keyword;
                if global {
                    inner.next();
                }
                let id = next_as_rc_string!(inner.next().unwrap().into_inner());
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(
                    span,
                    Node::Assign {
                        id,
                        expression,
                        global,
                    },
                )
            }
            Rule::multiple_assignment => {
                let mut inner = pair.into_inner();
                let global = inner.peek().unwrap().as_rule() == Rule::global_keyword;
                if global {
                    inner.next();
                }
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
                AstNode::new(
                    span,
                    Node::MultiAssign {
                        ids,
                        expressions,
                        global,
                    },
                )
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

                AstNode::new(
                    span,
                    Node::If {
                        condition,
                        then_node,
                        else_node,
                        else_if_condition: None,
                        else_if_node: None,
                    },
                )
            }
            Rule::if_block => {
                let mut inner = pair.into_inner();
                inner.next(); // if
                let condition = next_as_boxed_ast!(inner);
                let then_node = next_as_boxed_ast!(inner);

                let (else_if_condition, else_if_node) = if inner.peek().is_some()
                    && inner.peek().unwrap().as_rule() == Rule::else_if_block
                {
                    let mut inner = inner.next().unwrap().into_inner();
                    inner.next(); // else if
                    let condition = next_as_boxed_ast!(inner);
                    let node = next_as_boxed_ast!(inner);
                    (Some(condition), Some(node))
                } else {
                    (None, None)
                };

                let else_node = if inner.peek().is_some() {
                    let mut inner = inner.next().unwrap().into_inner();
                    Some(next_as_boxed_ast!(inner))
                } else {
                    None
                };

                AstNode::new(
                    span,
                    Node::If {
                        condition,
                        then_node,
                        else_if_condition,
                        else_if_node,
                        else_node,
                    },
                )
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
                AstNode::new(
                    span,
                    Node::For(Rc::new(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    })),
                )
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
                AstNode::new(
                    span,
                    Node::For(Rc::new(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    })),
                )
            }
            unexpected => unreachable!("Unexpected expression: {:?} - {:#?}", unexpected, pair),
        }
    }
}
