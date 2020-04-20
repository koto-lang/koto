use crate::{lookup::*, node::*, prec_climber::PrecClimber, AstNode, ConstantPool, LookupNode};
use pest::Parser;
use std::{collections::HashSet, convert::TryFrom, iter::FromIterator, rc::Rc};

use koto_grammar::Rule;

type Error = pest::error::Error<Rule>;

const TEMP_VAR_PREFIX: &str = "__";

#[derive(Debug, Default)]
struct LocalFunctionIds {
    // IDs that are available in the parent scope.
    ids_in_parent_scope: HashSet<ConstantIndex>,
    // IDs that have been assigned within the scope of a function.
    ids_assigned_in_function: HashSet<ConstantIndex>,
    // Captures are IDs and lookup roots, that are accessed in a function,
    // which haven't been yet assigned in the function,
    // but are available in the parent scope.
    captures: HashSet<ConstantIndex>,
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

    pub fn parse(&self, source: &str, constants: &mut ConstantPool) -> Result<AstNode, Error> {
        let mut parsed = koto_grammar::KotoParser::parse(Rule::program, source)?;

        Ok(self.build_ast(
            parsed.next().unwrap(),
            constants,
            &mut LocalFunctionIds::default(),
        ))
    }

    fn build_ast(
        &self,
        pair: pest::iterators::Pair<Rule>,
        constants: &mut ConstantPool,
        function_ids: &mut LocalFunctionIds,
    ) -> AstNode {
        use pest::iterators::Pair;

        macro_rules! next_as_boxed_ast {
            ($inner:expr) => {
                Box::new(self.build_ast($inner.next().unwrap(), constants, function_ids))
            };
        }

        macro_rules! pair_as_lookup {
            ($lookup_pair:expr) => {{
                let lookup = Lookup(
                    $lookup_pair
                        .into_inner()
                        .map(|pair| match pair.as_rule() {
                            Rule::single_id => {
                                LookupNode::Id(add_constant_string(constants, pair.as_str()))
                            }
                            Rule::map_access => LookupNode::Id(add_constant_string(
                                constants,
                                pair.into_inner().next().unwrap().as_str(),
                            )),
                            Rule::index => {
                                let mut inner = pair.into_inner();
                                let expression = next_as_boxed_ast!(inner);
                                LookupNode::Index(Index(expression))
                            }
                            Rule::call_args => {
                                let args = pair
                                    .into_inner()
                                    .map(|pair| self.build_ast(pair, constants, function_ids))
                                    .collect::<Vec<_>>();
                                LookupNode::Call(args)
                            }
                            unexpected => {
                                panic!("Unexpected rule while making lookup node: {:?}", unexpected)
                            }
                        })
                        .collect::<Vec<_>>(),
                );

                lookup
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
                    Rule::id => {
                        let id_index = add_constant_string(constants, next.as_str());
                        LookupOrId::Id(id_index)
                    }
                    Rule::lookup => LookupOrId::Lookup(pair_as_lookup!(next)),
                    _ => unreachable!(),
                }
            }};
        }

        let span = pair.as_span();
        match pair.as_rule() {
            Rule::next_expression => {
                self.build_ast(pair.into_inner().next().unwrap(), constants, function_ids)
            }
            Rule::program | Rule::child_block => {
                let inner = pair.into_inner();
                let block: Vec<AstNode> = inner
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect();
                AstNode::new(span, Node::Block(block))
            }
            Rule::expressions | Rule::value_terms => {
                let inner = pair.into_inner();
                let expressions = inner
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect::<Vec<_>>();

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
            Rule::number => {
                let n: f64 = pair.as_str().parse().unwrap();
                let constant_index = match u32::try_from(constants.add_f64(n)) {
                    Ok(index) => index,
                    Err(_) => panic!("The constant pool has overflowed"), // TODO Return an error
                };

                AstNode::new(span, Node::Number(constant_index))
            }
            Rule::string => AstNode::new(
                span,
                Node::Str(add_constant_string(
                    constants,
                    pair.into_inner().next().unwrap().as_str(),
                )),
            ),
            Rule::list => {
                let inner = pair.into_inner();
                let elements = inner
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect::<Vec<_>>();
                AstNode::new(span, Node::List(elements))
            }
            Rule::vec4_with_parens | Rule::vec4_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // vec4
                let expressions = inner
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect::<Vec<_>>();
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
                        let id = add_constant_string(constants, inner.next().unwrap().as_str());
                        let value = self.build_ast(inner.next().unwrap(), constants, function_ids);
                        (id, value)
                    })
                    .collect::<Vec<_>>();
                AstNode::new(span, Node::Map(entries))
            }
            Rule::negatable_lookup => {
                let mut inner = pair.into_inner();
                if inner.peek().unwrap().as_rule() == Rule::negative {
                    inner.next();
                    let lookup = next_as_lookup!(inner);
                    add_lookup_to_captures(function_ids, &lookup);
                    AstNode::new(
                        span.clone(),
                        Node::Negate(Box::new(AstNode::new(span, Node::Lookup(lookup)))),
                    )
                } else {
                    let lookup = next_as_lookup!(inner);
                    add_lookup_to_captures(function_ids, &lookup);
                    AstNode::new(span, Node::Lookup(lookup))
                }
            }
            Rule::id => {
                let mut inner = pair.into_inner();
                if inner.peek().unwrap().as_rule() == Rule::negative {
                    inner.next();
                    AstNode::new(span, Node::Negate(next_as_boxed_ast!(inner)))
                } else {
                    self.build_ast(inner.next().unwrap(), constants, function_ids)
                }
            }
            Rule::single_id => {
                let id_index = add_constant_string(constants, pair.as_str());
                add_id_to_captures(function_ids, id_index);
                AstNode::new(span, Node::Id(id_index))
            }
            Rule::copy_id => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let lookup_or_id = next_as_lookup_or_id!(inner);
                add_lookup_or_id_to_captures(function_ids, &lookup_or_id);
                AstNode::new(span, Node::Copy(lookup_or_id))
            }
            Rule::copy_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let expression = next_as_boxed_ast!(inner);
                AstNode::new(span, Node::CopyExpression(expression))
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
                    .map(|pair| add_constant_string(constants, pair.as_str()))
                    .collect::<Vec<_>>();

                let mut nested_function_ids = LocalFunctionIds::default();
                nested_function_ids.ids_in_parent_scope = function_ids
                    .ids_assigned_in_function
                    .union(&function_ids.ids_in_parent_scope)
                    .cloned()
                    .collect();
                nested_function_ids
                    .ids_assigned_in_function
                    .extend(args.clone());

                // collect function body
                let body: Vec<AstNode> = inner
                    .map(|pair| self.build_ast(pair, constants, &mut nested_function_ids))
                    .collect();

                let body = if body.len() == 1 {
                    vec![AstNode::new(
                        span.clone(),
                        Node::ReturnExpression(Box::new(body.first().unwrap().clone())),
                    )]
                } else {
                    body
                };

                // Captures from the nested function that are from this function's parent scope
                // need to be added to this function's captures.
                let missing_captures = nested_function_ids
                    .captures
                    .difference(&function_ids.ids_assigned_in_function);
                function_ids.captures.extend(missing_captures);

                AstNode::new(
                    span,
                    Node::Function(Rc::new(self::Function {
                        args,
                        captures: Vec::from_iter(nested_function_ids.captures),
                        body,
                    })),
                )
            }
            Rule::call_no_parens => {
                let mut inner = pair.into_inner();
                let function = next_as_lookup_or_id!(inner);
                add_lookup_or_id_to_captures(function_ids, &function);
                let args = inner
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect::<Vec<_>>();
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
                        (
                            add_constant_string(constants, pair.as_str()),
                            self.build_ast(pair, constants, function_ids),
                        )
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
                            id_index: add_constant_string(
                                constants,
                                inner.next().unwrap().as_str(),
                            ),
                            scope,
                        }
                    }
                    Rule::lookup => AssignTarget::Lookup(next_as_lookup!(inner)),
                    _ => unreachable!(),
                };
                let operator = inner.next().unwrap().as_rule();
                let rhs = next_as_boxed_ast!(inner);
                macro_rules! make_assign_op {
                    ($op:ident) => {{
                        add_assign_target_to_captures(function_ids, &target);
                        Box::new(AstNode::new(
                            span.clone(),
                            Node::Op {
                                op: AstOp::$op,
                                lhs: Box::new(AstNode::new(span.clone(), target.to_node())),
                                rhs,
                            },
                        ))
                    }};
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

                add_assign_target_to_ids_assigned_in_function(function_ids, &target);

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
                                id_index: add_constant_string(
                                    constants,
                                    inner.next().unwrap().as_str(),
                                ),
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
                    .map(|pair| self.build_ast(pair, constants, function_ids))
                    .collect::<Vec<_>>();

                for target in targets.iter() {
                    add_assign_target_to_ids_assigned_in_function(function_ids, target);
                }

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
                    |pair: Pair<Rule>| self.build_ast(pair, constants, function_ids),
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
                self.post_process_operation_tree(operation_tree, 0, constants)
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
                    .map(|pair| add_constant_string(constants, pair.as_str()))
                    .collect::<Vec<_>>();
                function_ids.ids_assigned_in_function.extend(args.clone());

                inner.next(); // in
                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(pair, constants, function_ids))
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
                let mut inner = pair.clone().into_inner();

                // To allow for loop values to captured in the inline body,
                // we skip ahead to evaluate the args first
                inner.next(); // body
                inner.next(); // for

                let args = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| add_constant_string(constants, pair.as_str()))
                    .collect::<Vec<_>>();
                function_ids.ids_assigned_in_function.extend(args.clone());

                let mut inner = pair.into_inner();
                let body = next_as_boxed_ast!(inner);

                inner.next(); // for
                inner.next(); // args have already been captured
                inner.next(); // in

                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(pair, constants, function_ids))
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

    fn post_process_operation_tree(
        &self,
        tree: AstNode,
        temp_value_counter: usize,
        constants: &mut ConstantPool,
    ) -> AstNode {
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
                                    let temp_value =
                                        format!("{}{}", TEMP_VAR_PREFIX, temp_value_counter);

                                    let constant_index =
                                        match u32::try_from(constants.add_string(&temp_value)) {
                                            Ok(index) => index,
                                            Err(_) => panic!("The constant pool has overflowed"),
                                        };

                                    Some(constant_index)
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
                                                    id_index: chained_temp_value,
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
                                        node: Node::Id(chained_temp_value),
                                        start_pos: tree.start_pos,
                                        end_pos: tree.end_pos,
                                    }),
                                    rhs,
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
                        op,
                        lhs: Box::new(self.post_process_operation_tree(
                            *lhs,
                            temp_value_counter + 1,
                            constants,
                        )),
                        rhs: Box::new(self.post_process_operation_tree(
                            *rhs,
                            temp_value_counter + 1,
                            constants,
                        )),
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

fn add_constant_string(constants: &mut ConstantPool, s: &str) -> u32 {
    match u32::try_from(constants.add_string(s)) {
        Ok(index) => index,
        Err(_) => panic!("The constant pool has overflowed"), // TODO Return an error
    }
}

fn add_assign_target_to_ids_assigned_in_function(
    function_ids: &mut LocalFunctionIds,
    target: &AssignTarget,
) {
    match target {
        AssignTarget::Id { id_index, .. } => {
            function_ids.ids_assigned_in_function.insert(*id_index);
        }
        AssignTarget::Lookup(lookup) => match lookup.as_slice().0 {
            &[LookupNode::Id(id_index), ..] => {
                function_ids.ids_assigned_in_function.insert(id_index);
            }
            _ => panic!("Expected Id as first lookup node"),
        },
    }
}

fn add_assign_target_to_captures(function_ids: &mut LocalFunctionIds, target: &AssignTarget) {
    match target {
        AssignTarget::Id { id_index, .. } => add_id_to_captures(function_ids, *id_index),
        AssignTarget::Lookup(lookup) => add_lookup_to_captures(function_ids, lookup),
    }
}

fn add_lookup_or_id_to_captures(function_ids: &mut LocalFunctionIds, lookup_or_id: &LookupOrId) {
    match lookup_or_id {
        LookupOrId::Id(id_index) => add_id_to_captures(function_ids, *id_index),
        LookupOrId::Lookup(lookup) => add_lookup_to_captures(function_ids, lookup),
    }
}

fn add_lookup_to_captures(function_ids: &mut LocalFunctionIds, lookup: &Lookup) {
    match lookup.as_slice().0 {
        &[LookupNode::Id(id_index), ..] => add_id_to_captures(function_ids, id_index),
        _ => panic!("Expected Id as first lookup node"),
    }
}

fn add_id_to_captures(function_ids: &mut LocalFunctionIds, id: ConstantIndex) {
    if !function_ids.ids_assigned_in_function.contains(&id)
        && function_ids.ids_in_parent_scope.contains(&id)
    {
        function_ids.captures.insert(id);
    }
}
