use crate::{lookup::*, node::*, prec_climber::PrecClimber, Ast, AstNode, LookupNode};
use pest::{error::Error, Parser};
use std::rc::Rc;

use koto_grammar::Rule;

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
            if pair.as_rule() == Rule::block {
                ast.push(self.build_ast(pair));
            }
        }

        Ok(ast)
    }

    fn build_ast(&self, pair: pest::iterators::Pair<Rule>) -> AstNode {
        use pest::iterators::Pair;

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

        macro_rules! pair_as_id {
            ($pair:expr) => {
                Rc::new($pair.as_str().to_string())
            };
        }

        macro_rules! pair_as_lookup {
            ($lookup_pair:expr) => {{
                match $lookup_pair.as_rule() {
                    Rule::index_start => Lookup(vec![{
                        let mut inner = $lookup_pair.into_inner();
                        let id = next_as_rc_string!(inner);
                        let expression = next_as_boxed_ast!(inner);
                        LookupNode::Index(Index {
                            id: Some(id),
                            expression,
                        })
                    }]),
                    Rule::lookup => Lookup(
                        $lookup_pair
                            .into_inner()
                            .map(|pair| match pair.as_rule() {
                                Rule::id => LookupNode::Id(pair_as_id!(pair)),
                                Rule::lookup_map => {
                                    let mut inner = pair.into_inner();
                                    LookupNode::Id(next_as_rc_string!(inner))
                                }
                                Rule::index_start => {
                                    let mut inner = pair.into_inner();
                                    let id = next_as_rc_string!(inner);
                                    let expression = next_as_boxed_ast!(inner);
                                    LookupNode::Index(Index {
                                        id: Some(id),
                                        expression,
                                    })
                                }
                                Rule::index_map => {
                                    let mut inner = pair.into_inner();
                                    let id = next_as_rc_string!(inner.next().unwrap().into_inner());
                                    let expression = next_as_boxed_ast!(inner);
                                    LookupNode::Index(Index {
                                        id: Some(id),
                                        expression,
                                    })
                                }
                                Rule::index_nested => {
                                    let mut inner = pair.into_inner();
                                    let expression = next_as_boxed_ast!(inner);
                                    LookupNode::Index(Index {
                                        id: None,
                                        expression,
                                    })
                                }
                                unexpected => panic!(
                                    "Unexpected rule while making lookup node: {:?}",
                                    unexpected
                                ),
                            })
                            .collect::<Vec<_>>(),
                    ),
                    unexpected => panic!(
                        "Unexpected rule while making lookup: {:?} - {:#?}",
                        unexpected, $lookup_pair
                    ),
                }
            }};
        }

        macro_rules! next_as_lookup {
            ($inner:expr) => {{
                let next = $inner.next().unwrap();
                pair_as_lookup!(next)
            }};
        }

        macro_rules! next_as_lookup_or_id {
            ($inner:expr) => {{
                let next = $inner.next().unwrap();
                match next.as_rule() {
                    Rule::id => LookupOrId::Id(pair_as_id!(next)),
                    Rule::lookup => LookupOrId::Lookup(pair_as_lookup!(next)),
                    _ => unreachable!(),
                }
            }};
        }

        let span = pair.as_span();
        match pair.as_rule() {
            Rule::next_expression => self.build_ast(pair.into_inner().next().unwrap()),
            Rule::block | Rule::child_block => {
                let inner = pair.into_inner();
                let block: Vec<AstNode> = inner.map(|pair| self.build_ast(pair)).collect();
                AstNode::new(span, Node::Block(block))
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
            Rule::boolean => (AstNode::new(span, Node::Bool(pair.as_str().parse().unwrap()))),
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
                let inner = if pair.as_rule() == Rule::map_value {
                    pair.into_inner().next().unwrap().into_inner()
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
            Rule::lookup => {
                let lookup = pair_as_lookup!(pair);
                AstNode::new(span, Node::Lookup(lookup))
            }
            Rule::id => {
                let id = Rc::new(pair.as_str().to_string());
                AstNode::new(span, Node::Id(id))
            }
            Rule::ref_id => {
                let mut inner = pair.into_inner();
                inner.next(); // ref
                let lookup_or_id = next_as_lookup_or_id!(inner);
                AstNode::new(span, Node::Ref(lookup_or_id))
            }
            Rule::ref_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // ref
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::RefExpression(expression))
            }
            Rule::negate => {
                let mut inner = pair.into_inner();
                inner.next(); // not
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::Negate(expression))
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
                let function = next_as_lookup_or_id!(inner);
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
                let target = match inner.peek().unwrap().as_rule() {
                    Rule::assignment_id => {
                        let mut inner = inner.next().unwrap().into_inner();

                        let scope = if inner.peek().unwrap().as_rule() == Rule::global_keyword {
                            inner.next();
                            Scope::Global
                        } else {
                            Scope::Local
                        };

                        AssignTarget::Id {
                            id: next_as_rc_string!(inner),
                            scope,
                        }
                    }
                    Rule::lookup => AssignTarget::Lookup(next_as_lookup!(inner)),
                    _ => unreachable!(),
                };
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::Assign { target, expression })
            }
            Rule::multiple_assignment => {
                let mut inner = pair.into_inner();
                let targets = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| match pair.as_rule() {
                        Rule::assignment_id => {
                            let mut inner = pair.into_inner();

                            let scope = if inner.peek().unwrap().as_rule() == Rule::global_keyword {
                                inner.next();
                                Scope::Global
                            } else {
                                Scope::Local
                            };

                            AssignTarget::Id {
                                id: next_as_rc_string!(inner),
                                scope,
                            }
                        }
                        Rule::lookup => AssignTarget::Lookup(pair_as_lookup!(pair)),
                        _ => unreachable!(),
                    })
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
                        targets,
                        expressions,
                    },
                )
            }
            Rule::operation => self.climber.climb(
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
            ),
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
                    Node::If(AstIf {
                        condition,
                        then_node,
                        else_node,
                        else_if_condition: None,
                        else_if_node: None,
                    }),
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
                    Node::If(AstIf {
                        condition,
                        then_node,
                        else_if_condition,
                        else_if_node,
                        else_node,
                    }),
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

impl Default for KotoParser {
    fn default() -> Self {
        Self::new()
    }
}
