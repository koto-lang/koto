use crate::{
    lookup::*, node::*, prec_climber::PrecClimber, Ast, AstIndex, AstNode, ConstantPool,
    LookupNode, ParserError, Position, Span,
};
use pest::Parser;
use std::{cell::RefCell, collections::HashSet, convert::TryFrom, iter::FromIterator};

use koto_grammar::Rule;

const TEMP_VAR_PREFIX: &str = "__";

impl<'a> From<pest::Span<'a>> for Span {
    fn from(span: pest::Span) -> Self {
        let start_line_col = span.start_pos().line_col();
        let end_line_col = span.end_pos().line_col();
        Self {
            start: Position {
                line: start_line_col.0 as u32,
                column: start_line_col.1 as u32,
            },
            end: Position {
                line: end_line_col.0 as u32,
                column: end_line_col.1 as u32,
            },
        }
    }
}

#[derive(Debug, Default)]
struct LocalIds {
    // IDs that are available in the parent scope.
    ids_in_parent_scope: HashSet<ConstantIndex>,
    // IDs that have been assigned within the current scope.
    ids_assigned_in_scope: HashSet<ConstantIndex>,
    // IDs that are currently being assigned to in the current scope.
    // We need to disinguish between 'has been assigned' and 'is being assigned' to allow capturing
    // of 'being assigned' values in child functions.
    // Once an ID has been marked as assigned locally it can't be captured, but if it isn't in
    // scope then it isn't made available to child functions.
    ids_being_assigned_in_scope: HashSet<ConstantIndex>,
    // Captures are IDs and lookup roots, that are accessed within the current scope,
    // which haven't yet been assigned in the current scope,
    // but are available in the parent scope.
    captures: HashSet<ConstantIndex>,
    // True if the scope is at the top level
    top_level: bool,
}

impl LocalIds {
    fn top_level() -> Self {
        Self {
            top_level: true,
            ..Default::default()
        }
    }

    fn all_available_ids(&self) -> HashSet<ConstantIndex> {
        let mut result = self
            .ids_assigned_in_scope
            .union(&self.ids_in_parent_scope)
            .cloned()
            .collect::<HashSet<_>>();
        result.extend(self.ids_being_assigned_in_scope.iter());
        result
    }

    fn local_count(&self) -> usize {
        self.ids_assigned_in_scope
            .difference(&self.captures)
            .count()
    }

    fn add_assign_target_to_ids_assigned_in_scope(&mut self, target: &AssignTarget) {
        match target {
            AssignTarget::Id { id_index, .. } => {
                self.ids_assigned_in_scope.insert(*id_index);
            }
            AssignTarget::Lookup(lookup) => match lookup.as_slice().0 {
                [LookupNode::Id(id_index), ..] => {
                    self.ids_assigned_in_scope.insert(*id_index);
                }
                _ => panic!("Expected Id as first lookup node"),
            },
        }
    }

    fn add_assign_target_to_ids_being_assigned_in_scope(&mut self, target: &AssignTarget) {
        match target {
            AssignTarget::Id { id_index, .. } => {
                self.ids_being_assigned_in_scope.insert(*id_index);
            }
            AssignTarget::Lookup(lookup) => match lookup.as_slice().0 {
                [LookupNode::Id(id_index), ..] => {
                    self.ids_being_assigned_in_scope.insert(*id_index);
                }
                _ => panic!("Expected Id as first lookup node"),
            },
        }
    }

    fn remove_assign_target_from_ids_being_assigned_in_scope(&mut self, target: &AssignTarget) {
        match target {
            AssignTarget::Id { id_index, .. } => {
                self.ids_being_assigned_in_scope.remove(id_index);
            }
            AssignTarget::Lookup(lookup) => match lookup.as_slice().0 {
                [LookupNode::Id(id_index), ..] => {
                    self.ids_being_assigned_in_scope.remove(id_index);
                }
                _ => panic!("Expected Id as first lookup node"),
            },
        }
    }

    fn add_assign_target_to_captures(&mut self, target: &AssignTarget) {
        match target {
            AssignTarget::Id { id_index, .. } => self.add_id_to_captures(*id_index),
            AssignTarget::Lookup(lookup) => self.add_lookup_to_captures(lookup),
        }
    }

    fn add_lookup_or_id_to_captures(&mut self, lookup_or_id: &LookupOrId) {
        match lookup_or_id {
            LookupOrId::Id(id_index) => self.add_id_to_captures(*id_index),
            LookupOrId::Lookup(lookup) => self.add_lookup_to_captures(lookup),
        }
    }

    fn add_lookup_to_captures(&mut self, lookup: &Lookup) {
        match lookup.as_slice().0 {
            &[LookupNode::Id(id_index), ..] => self.add_id_to_captures(id_index),
            _ => panic!("Expected Id as first lookup node"),
        }
    }

    fn add_id_to_captures(&mut self, id: ConstantIndex) {
        if !self.ids_assigned_in_scope.contains(&id) && self.ids_in_parent_scope.contains(&id) {
            self.captures.insert(id);
        }
    }
}

#[derive(Default)]
pub struct Options {
    pub export_all_top_level: bool,
}

pub struct KotoParser {
    climber: PrecClimber<Rule>,
    options: Options,
}

impl Default for KotoParser {
    fn default() -> Self {
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
            options: Default::default(),
        }
    }
}

impl KotoParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(
        &mut self,
        source: &str,
        constants: &mut ConstantPool,
        options: Options,
    ) -> Result<Ast, ParserError> {
        self.options = options;

        let token_count_guess = source.len() / 8;
        let mut ast = RefCell::new(Ast::with_capacity(token_count_guess));

        let mut parsed = koto_grammar::KotoParser::parse(Rule::program, source)
            .map_err(|pest_error| ParserError::PestSyntaxError(pest_error.to_string()))?;

        self.build_ast(
            &mut ast,
            parsed.next().unwrap(),
            constants,
            &mut LocalIds::top_level(),
        )?;

        Ok(ast.into_inner())
    }

    fn build_ast(
        &self,
        ast: &RefCell<Ast>,
        pair: pest::iterators::Pair<Rule>,
        constants: &mut ConstantPool,
        local_ids: &mut LocalIds,
    ) -> Result<AstIndex, ParserError> {
        use pest::iterators::Pair;

        macro_rules! build_next {
            ($inner:expr) => {
                self.build_ast(ast, $inner.next().unwrap(), constants, local_ids)?
            };
        }

        macro_rules! pair_as_lookup {
            ($lookup_pair:expr) => {{
                let lookup = Lookup(
                    $lookup_pair
                        .into_inner()
                        .map(|pair| match pair.as_rule() {
                            Rule::single_id => Ok(LookupNode::Id(add_constant_string(
                                constants,
                                pair.as_str(),
                            ))),
                            Rule::map_access => Ok(LookupNode::Id(add_constant_string(
                                constants,
                                pair.into_inner().next().unwrap().as_str(),
                            ))),
                            Rule::index => {
                                let mut inner = pair.into_inner();
                                let expression = build_next!(inner);
                                Ok(LookupNode::Index(Index(expression)))
                            }
                            Rule::call_args => {
                                let args = pair
                                    .into_inner()
                                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                                    .collect::<Result<Vec<_>, _>>()?;
                                Ok(LookupNode::Call(args))
                            }
                            unexpected => {
                                panic!("Unexpected rule while making lookup node: {:?}", unexpected)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?,
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
            Rule::next_expressions => {
                self.build_ast(ast, pair.into_inner().next().unwrap(), constants, local_ids)
            }
            Rule::program => {
                let inner = pair.into_inner();

                assert!(local_ids.ids_assigned_in_scope.is_empty());
                assert!(local_ids.captures.is_empty());

                let body = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;

                // the top level scope will have captures assigned to it due to the grandparenting
                // logic, but there's nothing to capture at the top level so clear the list to
                // ensure a correct local count.
                local_ids.captures.clear();
                let local_count = local_ids.local_count();

                let entry_point = ast
                    .borrow_mut()
                    .push(Node::MainBlock { body, local_count }, span.into())?;
                ast.borrow_mut().set_entry_point(entry_point);
                Ok(entry_point)
            }
            Rule::child_block => {
                let inner = pair.into_inner();
                let block = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut().push(Node::Block(block), span.into())
            }
            Rule::expressions | Rule::value_terms => {
                let inner = pair.into_inner();
                let expressions = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;

                if expressions.len() == 1 {
                    Ok(*expressions.first().unwrap())
                } else {
                    ast.borrow_mut()
                        .push(Node::Expressions(expressions), span.into())
                }
            }
            Rule::empty => ast.borrow_mut().push(Node::Empty, span.into()),
            Rule::boolean => {
                let bool_value: bool = pair.as_str().parse().unwrap();
                ast.borrow_mut().push(
                    if bool_value {
                        Node::BoolTrue
                    } else {
                        Node::BoolFalse
                    },
                    span.into(),
                )
            }
            Rule::number => {
                let n: f64 = pair.as_str().parse().unwrap();
                match n {
                    _ if n == 0.0 => ast.borrow_mut().push(Node::Number0, span.into()),
                    _ if n == 1.0 => ast.borrow_mut().push(Node::Number1, span.into()),
                    _ => {
                        let constant_index = match u32::try_from(constants.add_f64(n)) {
                            Ok(index) => index,
                            Err(_) => panic!("The constant pool has overflowed"),
                        };

                        ast.borrow_mut()
                            .push(Node::Number(constant_index), span.into())
                    }
                }
            }
            Rule::string => ast.borrow_mut().push(
                Node::Str(add_constant_string(
                    constants,
                    pair.into_inner().next().unwrap().as_str(),
                )),
                span.into(),
            ),
            Rule::list => {
                let inner = pair.into_inner();
                let elements = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut().push(Node::List(elements), span.into())
            }
            Rule::num2_with_parens | Rule::num2_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // num2
                let expressions = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut().push(Node::Num2(expressions), span.into())
            }
            Rule::num4_with_parens | Rule::num4_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // num4
                let expressions = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut().push(Node::Num4(expressions), span.into())
            }
            Rule::range => {
                let mut inner = pair.into_inner();

                let maybe_start = match inner.peek().unwrap().as_rule() {
                    Rule::range_op => None,
                    _ => Some(build_next!(inner)),
                };

                let inclusive = inner.next().unwrap().as_str() == "..=";

                let maybe_end = if inner.peek().is_some() {
                    Some(build_next!(inner))
                } else {
                    None
                };

                ast.borrow_mut().push(
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
                    span.into(),
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
                        let value =
                            self.build_ast(ast, inner.next().unwrap(), constants, local_ids)?;
                        Ok((id, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut().push(Node::Map(entries), span.into())
            }
            Rule::negatable_lookup => {
                let mut inner = pair.into_inner();
                if inner.peek().unwrap().as_rule() == Rule::negative {
                    inner.next();
                    let lookup = next_as_lookup!(inner);
                    local_ids.add_lookup_to_captures(&lookup);
                    let expression = ast
                        .borrow_mut()
                        .push(Node::Lookup(lookup), span.clone().into())?;
                    ast.borrow_mut().push(Node::Negate(expression), span.into())
                } else {
                    let lookup = next_as_lookup!(inner);
                    local_ids.add_lookup_to_captures(&lookup);
                    ast.borrow_mut().push(Node::Lookup(lookup), span.into())
                }
            }
            Rule::id => {
                let mut inner = pair.into_inner();
                if inner.peek().unwrap().as_rule() == Rule::negative {
                    inner.next();
                    let expression = build_next!(inner);
                    ast.borrow_mut().push(Node::Negate(expression), span.into())
                } else {
                    self.build_ast(ast, inner.next().unwrap(), constants, local_ids)
                }
            }
            Rule::single_id => {
                let id_index = add_constant_string(constants, pair.as_str());
                local_ids.add_id_to_captures(id_index);
                ast.borrow_mut().push(Node::Id(id_index), span.into())
            }
            Rule::copy_id => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let lookup_or_id = next_as_lookup_or_id!(inner);
                local_ids.add_lookup_or_id_to_captures(&lookup_or_id);
                ast.borrow_mut().push(Node::Copy(lookup_or_id), span.into())
            }
            Rule::copy_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // copy
                let expression = build_next!(inner);
                ast.borrow_mut()
                    .push(Node::CopyExpression(expression), span.into())
            }
            Rule::return_expression => {
                let mut inner = pair.into_inner();
                inner.next(); // return
                let node = if inner.peek().is_some() {
                    Node::ReturnExpression(build_next!(inner))
                } else {
                    Node::Return
                };
                ast.borrow_mut().push(node, span.into())
            }
            Rule::negate => {
                let mut inner = pair.into_inner();
                inner.next(); // not
                let expression = build_next!(inner);
                ast.borrow_mut().push(Node::Negate(expression), span.into())
            }
            Rule::function_block | Rule::function_inline => {
                let mut inner = pair.into_inner();
                let mut capture = inner.next().unwrap().into_inner();

                let args = capture
                    .by_ref()
                    .map(|pair| add_constant_string(constants, pair.as_str()))
                    .collect::<Vec<_>>();

                let is_instance_function = match args.as_slice() {
                    [first, ..] => constants.get_string(*first as usize) == "self",
                    _ => false,
                };

                let mut nested_local_ids = LocalIds::default();
                nested_local_ids.ids_in_parent_scope = local_ids.all_available_ids();
                nested_local_ids.ids_assigned_in_scope.extend(args.clone());

                // collect function body
                let body = inner
                    .map(|pair| self.build_ast(ast, pair, constants, &mut nested_local_ids))
                    .collect::<Result<Vec<_>, _>>()?;

                // Captures from the nested function that are from this function's parent scope
                // need to be added to this function's captures.
                let missing_captures = nested_local_ids
                    .captures
                    .difference(&local_ids.ids_assigned_in_scope);
                local_ids.captures.extend(missing_captures);

                let local_count = nested_local_ids.local_count();

                ast.borrow_mut().push(
                    Node::Function(self::Function {
                        args,
                        captures: Vec::from_iter(nested_local_ids.captures),
                        local_count,
                        body,
                        is_instance_function,
                    }),
                    span.into(),
                )
            }
            Rule::call_no_parens => {
                let mut inner = pair.into_inner();
                let function = next_as_lookup_or_id!(inner);
                local_ids.add_lookup_or_id_to_captures(&function);
                let args = inner
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                ast.borrow_mut()
                    .push(Node::Call { function, args }, span.into())
            }
            Rule::debug_with_parens | Rule::debug_no_parens => {
                let mut inner = pair.into_inner();
                inner.next(); // debug
                inner = inner.next().unwrap().into_inner();
                let expression_pair = inner.next().unwrap();

                let expression_string = add_constant_string(constants, expression_pair.as_str());
                let expression = self.build_ast(ast, expression_pair, constants, local_ids)?;

                ast.borrow_mut().push(
                    Node::Debug {
                        expression_string,
                        expression,
                    },
                    span.into(),
                )
            }
            Rule::single_assignment => {
                let mut inner = pair.into_inner();
                let target = match inner.peek().unwrap().as_rule() {
                    Rule::scoped_assign_id => {
                        let mut inner = inner.next().unwrap().into_inner();

                        let scope = if inner.peek().unwrap().as_rule() == Rule::export_keyword {
                            inner.next();
                            Scope::Global
                        } else if local_ids.top_level && self.options.export_all_top_level {
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

                local_ids.add_assign_target_to_captures(&target);
                local_ids.add_assign_target_to_ids_being_assigned_in_scope(&target);

                let rhs = build_next!(inner);
                macro_rules! make_assign_op {
                    ($op:ident) => {{
                        let lhs = ast
                            .borrow_mut()
                            .push(target.to_node(), span.clone().into())?;
                        ast.borrow_mut().push(
                            Node::Op {
                                op: AstOp::$op,
                                lhs,
                                rhs,
                            },
                            span.clone().into(),
                        )?
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

                // TODO only set as assigned locally when in local scope
                // Add the target to the assigned list
                local_ids.remove_assign_target_from_ids_being_assigned_in_scope(&target);
                local_ids.add_assign_target_to_ids_assigned_in_scope(&target);

                ast.borrow_mut()
                    .push(Node::Assign { target, expression }, span.into())
            }
            Rule::multiple_assignment => {
                let mut inner = pair.into_inner();
                let targets = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| match pair.as_rule() {
                        Rule::scoped_assign_id => {
                            let mut inner = pair.into_inner();

                            let scope = if inner.peek().unwrap().as_rule() == Rule::export_keyword {
                                inner.next();
                                Scope::Global
                            } else if local_ids.top_level && self.options.export_all_top_level {
                                Scope::Global
                            } else {
                                Scope::Local
                            };

                            Ok(AssignTarget::Id {
                                id_index: add_constant_string(
                                    constants,
                                    inner.next().unwrap().as_str(),
                                ),
                                scope,
                            })
                        }
                        Rule::lookup => Ok(AssignTarget::Lookup(pair_as_lookup!(pair))),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                for target in targets.iter() {
                    local_ids.add_assign_target_to_ids_being_assigned_in_scope(target);
                }

                let expressions = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;

                for target in targets.iter() {
                    local_ids.remove_assign_target_from_ids_being_assigned_in_scope(target);
                    local_ids.add_assign_target_to_ids_assigned_in_scope(target);
                }

                ast.borrow_mut().push(
                    Node::MultiAssign {
                        targets,
                        expressions,
                    },
                    span.into(),
                )
            }
            Rule::operation => {
                let operation_tree = self.climber.climb(
                    pair.into_inner(),
                    |pair: Pair<Rule>| self.build_ast(ast, pair, constants, local_ids),
                    |lhs: Result<AstIndex, ParserError>,
                     op: Pair<Rule>,
                     rhs: Result<AstIndex, ParserError>| {
                        use AstOp::*;

                        let span = op.as_span();
                        let lhs = lhs?;
                        let rhs = rhs?;

                        macro_rules! make_ast_op {
                            ($op:expr) => {
                                ast.borrow_mut()
                                    .push(Node::Op { op: $op, lhs, rhs }, span.into())
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
                                Err(ParserError::ParserError(error))
                            }
                        }
                    },
                )?;
                self.post_process_operation_tree(ast, operation_tree, 0, constants, local_ids)
            }
            Rule::if_inline => {
                let mut inner = pair.into_inner();
                inner.next(); // if
                let condition = build_next!(inner);
                inner.next(); // then
                let then_node = build_next!(inner);
                let else_node = if inner.next().is_some() {
                    Some(build_next!(inner))
                } else {
                    None
                };

                ast.borrow_mut().push(
                    Node::If(AstIf {
                        condition,
                        then_node,
                        else_if_blocks: vec![],
                        else_node,
                    }),
                    span.into(),
                )
            }
            Rule::if_block => {
                let mut inner = pair.into_inner();
                inner.next(); // if
                let condition = build_next!(inner);
                let then_node = build_next!(inner);

                let mut else_if_blocks = Vec::new();

                while inner.peek().is_some()
                    && inner.peek().unwrap().as_rule() == Rule::else_if_block
                {
                    let mut inner = inner.next().unwrap().into_inner();
                    inner.next(); // else if
                    let condition = build_next!(inner);
                    let node = build_next!(inner);
                    else_if_blocks.push((condition, node));
                }

                let else_node = if inner.peek().is_some() {
                    let mut inner = inner.next().unwrap().into_inner();
                    inner.next(); // else
                    Some(build_next!(inner))
                } else {
                    None
                };

                ast.borrow_mut().push(
                    Node::If(AstIf {
                        condition,
                        then_node,
                        else_if_blocks,
                        else_node,
                    }),
                    span.into(),
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
                local_ids.ids_assigned_in_scope.extend(args.clone());

                inner.next(); // in
                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                let condition = if inner.peek().unwrap().as_rule() == Rule::if_keyword {
                    inner.next();
                    Some(build_next!(inner))
                } else {
                    None
                };

                let body = build_next!(inner);
                ast.borrow_mut().push(
                    Node::For(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    }),
                    span.into(),
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
                local_ids.ids_assigned_in_scope.extend(args.clone());

                let mut inner = pair.into_inner();
                let body = build_next!(inner);

                inner.next(); // for
                inner.next(); // args have already been captured
                inner.next(); // in

                let ranges = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|pair| self.build_ast(ast, pair, constants, local_ids))
                    .collect::<Result<Vec<_>, _>>()?;
                let condition = if inner.next().is_some() {
                    // if
                    Some(build_next!(inner))
                } else {
                    None
                };
                ast.borrow_mut().push(
                    Node::For(AstFor {
                        args,
                        ranges,
                        condition,
                        body,
                    }),
                    span.into(),
                )
            }
            Rule::while_block => {
                let mut inner = pair.into_inner();
                let negate_condition = match inner.next().unwrap().as_rule() {
                    Rule::while_keyword => false,
                    Rule::until_keyword => true,
                    _ => unreachable!(),
                };
                let condition = build_next!(inner);
                let body = build_next!(inner);
                ast.borrow_mut().push(
                    Node::While(AstWhile {
                        condition,
                        body,
                        negate_condition,
                    }),
                    span.into(),
                )
            }
            Rule::while_inline => {
                let mut inner = pair.into_inner();
                let body = build_next!(inner);
                let negate_condition = match inner.next().unwrap().as_rule() {
                    Rule::while_keyword => false,
                    Rule::until_keyword => true,
                    _ => unreachable!(),
                };
                let condition = build_next!(inner);
                ast.borrow_mut().push(
                    Node::While(AstWhile {
                        condition,
                        body,
                        negate_condition,
                    }),
                    span.into(),
                )
            }
            Rule::break_ => ast.borrow_mut().push(Node::Break, span.into()),
            Rule::continue_ => ast.borrow_mut().push(Node::Continue, span.into()),
            unexpected => unreachable!("Unexpected expression: {:?} - {:#?}", unexpected, pair),
        }
    }

    fn post_process_operation_tree(
        &self,
        ast: &RefCell<Ast>,
        tree_index: AstIndex,
        temp_value_counter: usize,
        constants: &mut ConstantPool,
        local_ids: &mut LocalIds,
    ) -> Result<AstIndex, ParserError> {
        // To support chained comparisons:
        //   if the node is an op
        //     and if the op is a comparison
        //       and if the lhs is also a comparison
        //         then convert the node to an And
        //           ..with lhs as the And's lhs, but with its rhs assigning to a temp value
        //           ..with rhs as the node's op, and with its lhs reading from the temp value
        //     then proceed down lhs and rhs
        let node = ast.borrow().node(tree_index).clone();
        match &node {
            AstNode {
                node: Node::Op { op, lhs, rhs },
                span,
            } => {
                use AstOp::*;
                let (op, lhs, rhs) = match op {
                    Greater | GreaterOrEqual | Less | LessOrEqual | Equal => {
                        let chained_temp_value = match ast.borrow().node(*lhs).node {
                            Node::Op { op, .. } => match op {
                                Greater | GreaterOrEqual | Less | LessOrEqual | Equal => {
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
                            let lhs_node = ast.borrow().node(*lhs).clone();
                            let lhs = match lhs_node.node {
                                Node::Op {
                                    op: lhs_op,
                                    lhs: lhs_lhs,
                                    rhs: lhs_rhs,
                                } => {
                                    let rhs = ast.borrow_mut().push_with_span_index(
                                        Node::Assign {
                                            target: AssignTarget::Id {
                                                id_index: chained_temp_value,
                                                scope: Scope::Local,
                                            },
                                            expression: lhs_rhs.clone(),
                                        },
                                        *span,
                                    )?;

                                    ast.borrow_mut().push_with_span_index(
                                        Node::Op {
                                            op: lhs_op,
                                            lhs: lhs_lhs,
                                            rhs,
                                        },
                                        *span,
                                    )?
                                }
                                _ => unreachable!(),
                            };

                            local_ids.ids_assigned_in_scope.insert(chained_temp_value);

                            // rewrite the rhs to perform the node's comparison,
                            // reading from the temp value on its lhs
                            let temp = ast
                                .borrow_mut()
                                .push_with_span_index(Node::Id(chained_temp_value), *span)?;
                            let rhs = ast.borrow_mut().push_with_span_index(
                                Node::Op {
                                    op: *op,
                                    lhs: temp,
                                    rhs: *rhs,
                                },
                                *span,
                            )?;

                            // Insert an And to chain the comparisons
                            (AstOp::And, lhs, rhs)
                        } else {
                            (*op, *lhs, *rhs)
                        }
                    }
                    _ => (*op, *lhs, *rhs),
                };

                let lhs = self.post_process_operation_tree(
                    ast,
                    lhs,
                    temp_value_counter + 1,
                    constants,
                    local_ids,
                )?;
                let rhs = self.post_process_operation_tree(
                    ast,
                    rhs,
                    temp_value_counter + 1,
                    constants,
                    local_ids,
                )?;

                ast.borrow_mut()
                    .push_with_span_index(Node::Op { op, lhs, rhs }, *span)
            }
            _ => Ok(tree_index),
        }
    }
}

fn add_constant_string(constants: &mut ConstantPool, s: &str) -> u32 {
    match u32::try_from(constants.add_string(s)) {
        Ok(index) => index,
        Err(_) => panic!("The constant pool has overflowed"), // TODO Return an error
    }
}
