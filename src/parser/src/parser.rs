use crate::{lookup::*, node::*, prec_climber::PrecClimber, AstNode, LookupNode};
use pest::Parser;
use std::rc::Rc;

use koto_grammar::Rule;

type Error = pest::error::Error<Rule>;

const TEMP_VAR_PREFIX: &str = "__";

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

    pub fn parse(&self, source: &str) -> Result<AstNode, Error> {
        let mut parsed = koto_grammar::KotoParser::parse(Rule::program, source)?;

        Ok(self.build_ast(parsed.next().unwrap()))
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
                Lookup(
                    $lookup_pair
                        .into_inner()
                        .map(|pair| match pair.as_rule() {
                            Rule::id => LookupNode::Id(pair_as_id!(pair)),
                            Rule::map_access => {
                                let mut inner = pair.into_inner();
                                LookupNode::Id(next_as_rc_string!(inner))
                            }
                            Rule::index => {
                                let mut inner = pair.into_inner();
                                let expression = next_as_boxed_ast!(inner);
                                LookupNode::Index(Index(expression))
                            }
                            Rule::call_args => {
                                let args = pair
                                    .into_inner()
                                    .map(|pair| self.build_ast(pair))
                                    .collect::<Vec<_>>();
                                LookupNode::Call(args)
                            }
                            unexpected => {
                                panic!("Unexpected rule while making lookup node: {:?}", unexpected)
                            }
                        })
                        .collect::<Vec<_>>(),
                )
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
            Rule::program | Rule::child_block => {
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
            Rule::empty => AstNode::new(span, Node::Empty),
            Rule::boolean => {
                let bool_value: bool = pair.as_str().parse().unwrap();
                AstNode::new(
                    span,
                    if bool_value {
                        Node::BoolTrue
                    } else {
                        Node::BoolFalse
                    },
                )
            }
            Rule::number => AstNode::new(span, Node::Number(pair.as_str().parse().unwrap())),
            Rule::string => {
                let mut inner = pair.into_inner();
                AstNode::new(span, Node::Str(next_as_rc_string!(inner)))
            }
            Rule::list => {
                let inner = pair.into_inner();
                let elements = inner.map(|pair| self.build_ast(pair)).collect::<Vec<_>>();
                AstNode::new(span, Node::List(elements))
            }
            Rule::vec4_with_parens | Rule::vec4_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // vec4
                let expressions = inner.map(|pair| self.build_ast(pair)).collect::<Vec<_>>();
                AstNode::new(span, Node::Vec4(expressions))
            }
            Rule::range => {
                let mut inner = pair.into_inner();

                let maybe_start = match inner.peek().unwrap().as_rule() {
                    Rule::range_op => None,
                    _ => Some(next_as_boxed_ast!(inner)),
                };

                let inclusive = inner.next().unwrap().as_str() == "..=";

                let maybe_end = if inner.peek().is_some() {
                    Some(next_as_boxed_ast!(inner))
                } else {
                    None
                };

                AstNode::new(
                    span,
                    match (&maybe_start, &maybe_end) {
                        (Some(start), Some(end)) => Node::Range {
                            start: start.clone(),
                            end: end.clone(),
                            inclusive,
                        },
                        (Some(start), None) => Node::RangeFrom {
                            start: start.clone(),
                        },
                        (None, Some(end)) => Node::RangeTo {
                            end: end.clone(),
                            inclusive,
                        },
                        _ => Node::RangeFull,
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
            Rule::copy_id => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let lookup_or_id = next_as_lookup_or_id!(inner);
                AstNode::new(span, Node::Copy(lookup_or_id))
            }
            Rule::copy_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::CopyExpression(expression))
            }
            Rule::share_id => {
                let mut inner = pair.into_inner();
                inner.next(); // share
                let lookup_or_id = next_as_lookup_or_id!(inner);
                AstNode::new(span, Node::Share(lookup_or_id))
            }
            Rule::share_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // share
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::ShareExpression(expression))
            }
            Rule::return_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // return
                AstNode::new(
                    span,
                    if inner.peek().is_some() {
                        Node::ReturnExpression(next_as_boxed_ast!(inner))
                    } else {
                        Node::Return
                    },
                )
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

                let body = if body.len() == 1 {
                    vec![AstNode::new(
                        span.clone(),
                        Node::ReturnExpression(Box::new(body.first().unwrap().clone())),
                    )]
                } else {
                    body
                };

                AstNode::new(span, Node::Function(Rc::new(self::Function { args, body })))
            }
            Rule::call_no_parens => {
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
            Rule::debug_with_parens | Rule::debug_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // debug
                let expressions = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| {
                        let text = pair.as_str().to_string();
                        (text, self.build_ast(pair))
                    })
                    .collect::<Vec<_>>();
                AstNode::new(span, Node::Debug { expressions })
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
                let operator = inner.next().unwrap().as_rule();
                let rhs = next_as_boxed_ast!(inner);
                macro_rules! make_assign_op {
                    ($op:ident) => {
                        Box::new(AstNode::new(
                            span.clone(),
                            Node::Op {
                                op: AstOp::$op,
                                lhs: Box::new(AstNode::new(span.clone(), target.to_node())),
                                rhs,
                            },
                        ))
                    };
                };
                let expression = match operator {
                    Rule::assign => rhs,
                    Rule::assign_add => make_assign_op!(Add),
                    Rule::assign_subtract => make_assign_op!(Subtract),
                    Rule::assign_multiply => make_assign_op!(Multiply),
                    Rule::assign_divide => make_assign_op!(Divide),
                    Rule::assign_modulo => make_assign_op!(Modulo),
                    _ => unreachable!(),
                };
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
            Rule::operation => {
                let operation_tree = self.climber.climb(
                    pair.into_inner(),
                    |pair: Pair<Rule>| self.build_ast(pair),
                    |lhs: AstNode, op: Pair<Rule>, rhs: AstNode| {
                        use AstOp::*;

                        let span = op.as_span();
                        let lhs = Box::new(lhs);
                        let rhs = Box::new(rhs);

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
                );
                self.post_process_operation_tree(operation_tree, 0)
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
                    inner.next(); // else
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
            Rule::while_loop => {
                let mut inner = pair.into_inner();
                let negate_condition = match inner.next().unwrap().as_rule() {
                    Rule::while_keyword => false,
                    Rule::until_keyword => true,
                    _ => unreachable!(),
                };
                let condition = next_as_boxed_ast!(inner);
                let body = next_as_boxed_ast!(inner);
                AstNode::new(
                    span,
                    Node::While(Rc::new(AstWhile {
                        condition,
                        body,
                        negate_condition,
                    })),
                )
            }
            Rule::break_ => AstNode::new(span, Node::Break),
            Rule::continue_ => AstNode::new(span, Node::Continue),
            unexpected => unreachable!("Unexpected expression: {:?} - {:#?}", unexpected, pair),
        }
    }

    fn post_process_operation_tree(&self, tree: AstNode, temp_value_counter: usize) -> AstNode {
        // To support chained comparisons:
        //   if the node is an op
        //     and if the op is a comparison
        //       and if the lhs is also a comparison
        //         then convert the node to an And
        //           ..with lhs as the And's lhs, but with its rhs assigning to a temp value
        //           ..with rhs as the node's op, and with its lhs reading from the temp value
        //     then proceed down lhs and rhs
        match tree.node {
            Node::Op { op, lhs, rhs } => {
                use AstOp::*;
                let (op, lhs, rhs) = match op {
                    Greater | GreaterOrEqual | Less | LessOrEqual => {
                        let chained_temp_value = match &lhs.node {
                            Node::Op { op, .. } => match op {
                                Greater | GreaterOrEqual | Less | LessOrEqual => {
                                    let temp_value = Id::new(format!(
                                        "{}{}",
                                        TEMP_VAR_PREFIX, temp_value_counter
                                    ));
                                    Some(temp_value)
                                }
                                _ => None,
                            },
                            _ => None,
                        };

                        if let Some(chained_temp_value) = chained_temp_value {
                            // rewrite the lhs to have its rhs assigned to a temp value
                            let lhs = match lhs.node {
                                Node::Op {
                                    op: lhs_op,
                                    lhs: lhs_lhs,
                                    rhs: lhs_rhs,
                                } => Box::new(AstNode {
                                    node: Node::Op {
                                        op: lhs_op,
                                        lhs: lhs_lhs,
                                        rhs: Box::new(AstNode {
                                            node: Node::Assign {
                                                target: AssignTarget::Id {
                                                    id: chained_temp_value.clone(),
                                                    scope: Scope::Local,
                                                },
                                                expression: lhs_rhs.clone(),
                                            },
                                            ..*lhs_rhs
                                        }),
                                    },
                                    ..*lhs
                                }),
                                _ => unreachable!(),
                            };

                            // rewrite the rhs to perform the node's comparison,
                            // reading from the temp value on its lhs
                            let rhs = Box::new(AstNode {
                                node: Node::Op {
                                    op,
                                    lhs: Box::new(AstNode {
                                        node: Node::Id(chained_temp_value.clone()),
                                        start_pos: tree.start_pos,
                                        end_pos: tree.end_pos,
                                    }),
                                    rhs: rhs,
                                },
                                start_pos: tree.start_pos,
                                end_pos: tree.end_pos,
                            });

                            // Insert an And to chain the comparisons
                            (AstOp::And, lhs, rhs)
                        } else {
                            (op, lhs, rhs)
                        }
                    }
                    _ => (op, lhs, rhs),
                };

                AstNode {
                    node: Node::Op {
                        op: op,
                        lhs: Box::new(
                            self.post_process_operation_tree(*lhs, temp_value_counter + 1),
                        ),
                        rhs: Box::new(
                            self.post_process_operation_tree(*rhs, temp_value_counter + 1),
                        ),
                    },
                    ..tree
                }
            }
            node => AstNode { node, ..tree },
        }
    }
}

impl Default for KotoParser {
    fn default() -> Self {
        Self::new()
    }
}
