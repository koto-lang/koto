#![cfg_attr(feature = "panic_on_parser_error", allow(unreachable_code))]

use {
    crate::{constant_pool::ConstantPoolBuilder, error::*, *},
    koto_lexer::{Lexer, Span, Token},
    std::{
        collections::{HashMap, HashSet},
        iter::FromIterator,
        str::FromStr,
    },
};

macro_rules! make_internal_error {
    ($error:ident, $parser:expr) => {{
        ParserError::new(InternalError::$error.into(), $parser.lexer.span())
    }};
}

macro_rules! internal_error {
    ($error:ident, $parser:expr) => {{
        let error = make_internal_error!($error, $parser);

        #[cfg(feature = "panic_on_parser_error")]
        panic!(error);

        #[cfg(not(feature = "panic_on_parser_error"))]
        Err(error)
    }};
}

macro_rules! parser_error {
    ($error:ident, $parser:expr, $error_type:ident) => {{
        let error = ParserError::new($error_type::$error.into(), $parser.lexer.span());

        #[cfg(feature = "panic_on_parser_error")]
        panic!(error);

        #[cfg(not(feature = "panic_on_parser_error"))]
        Err(error)
    }};
}

macro_rules! indentation_error {
    ($error:ident, $parser:expr) => {{
        parser_error!($error, $parser, ExpectedIndentation)
    }};
}

macro_rules! syntax_error {
    ($error:ident, $parser:expr) => {{
        parser_error!($error, $parser, SyntaxError)
    }};
}

fn f64_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < std::f64::EPSILON
}

enum ConstantIndexOrWildcard {
    Index(ConstantIndex),
    Wildcard,
}

#[derive(Debug, Default)]
struct Frame {
    // If a frame contains yield then it represents a generator function
    contains_yield: bool,
    // IDs that have been assigned within the current frame
    ids_assigned_in_scope: HashSet<ConstantIndex>,
    // IDs and lookup roots which have been accessed without being locally assigned previously
    accessed_non_locals: HashSet<ConstantIndex>,
    // Due to single-pass parsing, we don't know while parsing an ID if it's the lhs of an
    // assignment or not. We have to wait until the expression is complete to determine if the
    // reference to an ID was non-local or not.
    // To achieve this, while an expression is being parsed we can maintain a running count:
    // +1 for reading, -1 for assignment. At the end of the expression, a positive count indicates
    // a non-local access.
    //
    // e.g.
    //
    // a is a local, it's on lhs of expression, so its a local assignment.
    // || a = 1
    // (access count == 0: +1 -1)
    //
    // a is first accessed as a non-local before being assigned locally.
    // || a = a
    // (access count == 1: +1 -1 +1)
    //
    // a is assigned locally twice from a non-local.
    // || a = a = a
    // (access count == 1: +1 -1 +1 -1 +1)
    //
    // a is a non-local in the inner frame, so is also a non-local in the outer frame
    // (|| a) for b in 0..10
    // (access count of a == 1: +1)
    //
    // a is a non-local in the inner frame, but a local in the outer frame.
    // The inner-frame non-local access counts as an outer access,
    // but the loop arg is a local assignment so the non-local access doesn't need to propagate out.
    // (|| a) for a in 0..10
    // (access count == 0: +1 -1)
    expression_id_accesses: HashMap<ConstantIndex, usize>,
}

impl Frame {
    // Locals in a frame are assigned values that weren't first accessed non-locally
    fn local_count(&self) -> usize {
        self.ids_assigned_in_scope
            .difference(&self.accessed_non_locals)
            .count()
    }

    // Non-locals accessed in a nested frame need to be declared as also accessed in this
    // frame. This ensures that captures from the outer frame will be available when
    // creating the nested inner scope.
    fn add_nested_accessed_non_locals(&mut self, nested_frame: &Frame) {
        for non_local in nested_frame.accessed_non_locals.iter() {
            self.increment_expression_access_for_id(*non_local);
        }
    }

    fn increment_expression_access_for_id(&mut self, id: ConstantIndex) {
        *self.expression_id_accesses.entry(id).or_insert(0) += 1;
    }

    fn decrement_expression_access_for_id(&mut self, id: ConstantIndex) -> Result<(), ()> {
        match self.expression_id_accesses.get_mut(&id) {
            Some(entry) => {
                *entry -= 1;
                Ok(())
            }
            None => Err(()),
        }
    }

    fn finish_expressions(&mut self) {
        for (id, access_count) in self.expression_id_accesses.iter() {
            if *access_count > 0 && !self.ids_assigned_in_scope.contains(id) {
                self.accessed_non_locals.insert(*id);
            }
        }
        self.expression_id_accesses.clear();
    }
}

#[derive(Clone, Copy, Debug)]
struct ExpressionContext {
    // e.g. a = f x y
    // `x` and `y` are `f`'s arguments, and while parsing them this flag is set to false,
    // preventing further function calls from being started.
    allow_space_separated_call: bool,
    // e.g. f = |x|
    //        x + x
    // This function can have an indented body.
    //
    // foo
    //   bar
    //   baz
    // This function call can be broken over lines.
    //
    // while x < f y
    //   ...
    // Here, `f y` can't be broken over lines as the while expression expects an indented block.
    allow_linebreaks: bool,
    // x =
    //   foo, bar
    //
    // `x` is at the start of a line, so it doesn't make sense to allow indentation.
    // `foo, bar` is to the right of an assignment so indentation is allowed.
    allow_initial_indentation: bool,
    // When None, then some indentation on following lines is expected.
    // When Some, then indentation should match the expected indentation.
    expected_indentation: Option<usize>,
}

impl ExpressionContext {
    fn line_start() -> Self {
        Self {
            allow_space_separated_call: true,
            allow_linebreaks: true,
            allow_initial_indentation: false,
            expected_indentation: None,
        }
    }

    fn permissive() -> Self {
        Self {
            allow_space_separated_call: true,
            allow_linebreaks: true,
            allow_initial_indentation: true,
            expected_indentation: None,
        }
    }

    fn restricted() -> Self {
        Self {
            allow_space_separated_call: false,
            allow_linebreaks: false,
            allow_initial_indentation: false,
            expected_indentation: None,
        }
    }

    fn inline() -> Self {
        Self {
            allow_space_separated_call: true,
            allow_linebreaks: false,
            allow_initial_indentation: false,
            expected_indentation: None,
        }
    }
}

pub struct Parser<'source> {
    ast: Ast,
    constants: ConstantPoolBuilder,
    lexer: Lexer<'source>,
    frame_stack: Vec<Frame>,
}

impl<'source> Parser<'source> {
    pub fn parse(source: &'source str) -> Result<(Ast, ConstantPool), ParserError> {
        let capacity_guess = source.len() / 4;
        let mut parser = Parser {
            ast: Ast::with_capacity(capacity_guess),
            constants: ConstantPoolBuilder::new(),
            lexer: Lexer::new(source),
            frame_stack: Vec::new(),
        };

        let main_block = parser.parse_main_block()?;
        parser.ast.set_entry_point(main_block);

        Ok((parser.ast, parser.constants.build()))
    }

    fn frame(&self) -> Result<&Frame, ParserError> {
        match self.frame_stack.last() {
            Some(frame) => Ok(frame),
            None => Err(ParserError::new(
                InternalError::MissingScope.into(),
                Span::default(),
            )),
        }
    }

    fn frame_mut(&mut self) -> Result<&mut Frame, ParserError> {
        match self.frame_stack.last_mut() {
            Some(frame) => Ok(frame),
            None => Err(ParserError::new(
                InternalError::MissingScope.into(),
                Span::default(),
            )),
        }
    }

    fn parse_main_block(&mut self) -> Result<AstIndex, ParserError> {
        self.frame_stack.push(Frame::default());

        let start_span = self.lexer.span();

        let mut context = ExpressionContext::line_start();
        context.expected_indentation = Some(0);

        let mut body = Vec::new();
        while self.peek_next_token(&context).is_some() {
            self.consume_until_next_token(&mut context);

            if let Some(expression) = self.parse_line()? {
                body.push(expression);
            } else {
                self.lexer.next();
                return syntax_error!(ExpectedExpressionInMainBlock, self);
            }
        }

        // Check that all tokens were consumed
        if self
            .peek_next_token(&ExpressionContext::permissive())
            .is_some()
        {
            return syntax_error!(UnexpectedToken, self);
        }

        let result = self.push_node_with_start_span(
            Node::MainBlock {
                body,
                local_count: self.frame()?.local_count(),
            },
            start_span,
        )?;

        self.frame_stack.pop();
        Ok(result)
    }

    fn parse_function(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let start_indent = self.lexer.current_indent();

        if self.consume_next_token_on_same_line() != Some(Token::Function) {
            return internal_error!(FunctionParseFailure, self);
        }

        let span_start = self.lexer.span().start;

        // Parse function's args
        let mut args = Vec::new();
        let mut is_instance_function = false;
        let mut is_variadic = false;

        let mut args_context = ExpressionContext::permissive();
        while self.peek_next_token(&args_context).is_some() {
            self.consume_until_next_token(&mut args_context);
            match self.parse_id_or_wildcard(context) {
                Some(ConstantIndexOrWildcard::Index(constant_index)) => {
                    if self.constants.pool().get_str(constant_index) == "self" {
                        if !args.is_empty() {
                            return syntax_error!(SelfArgNotInFirstPosition, self);
                        }
                        is_instance_function = true;
                    }

                    args.push(Some(constant_index));

                    if self.peek_token() == Some(Token::Ellipsis) {
                        self.consume_token();
                        is_variadic = true;
                        break;
                    }
                }
                Some(ConstantIndexOrWildcard::Wildcard) => args.push(None),
                None => break,
            }

            if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                self.consume_next_token_on_same_line();
            } else {
                break;
            }
        }

        // Check for function args end
        let mut function_end_context = ExpressionContext::permissive();
        function_end_context.expected_indentation = Some(start_indent);
        if self.consume_next_token(&mut function_end_context) != Some(Token::Function) {
            return syntax_error!(ExpectedFunctionArgsEnd, self);
        }

        // body
        let mut function_frame = Frame::default();
        function_frame
            .ids_assigned_in_scope
            .extend(args.iter().cloned().filter_map(|maybe_id| maybe_id));
        self.frame_stack.push(function_frame);

        let body = if let Some(block) = self.parse_indented_map_or_block()? {
            // If the body is a Map block, then finish_expressions is needed here to finalise the
            // captures for the Map values. Normally parse_line takes care of calling
            // finish_expressions, but this is a situation where it can be bypassed.
            self.frame_mut()?.finish_expressions();
            block
        } else {
            self.consume_until_next_token_on_same_line();
            if let Some(body) = self.parse_line()? {
                body
            } else {
                return indentation_error!(ExpectedFunctionBody, self);
            }
        };

        let function_frame = self
            .frame_stack
            .pop()
            .ok_or_else(|| make_internal_error!(MissingScope, self))?;

        self.frame_mut()?
            .add_nested_accessed_non_locals(&function_frame);

        let local_count = function_frame.local_count();

        let span_end = self.lexer.span().end;

        let result = self.ast.push(
            Node::Function(Function {
                args,
                local_count,
                accessed_non_locals: Vec::from_iter(function_frame.accessed_non_locals),
                body,
                is_instance_function,
                is_variadic,
                is_generator: function_frame.contains_yield,
            }),
            Span {
                start: span_start,
                end: span_end,
            },
        )?;

        Ok(Some(result))
    }

    fn parse_line(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let result = if let Some(for_loop) =
            self.parse_for_loop(&mut ExpressionContext::line_start())?
        {
            Some(for_loop)
        } else if let Some(loop_block) = self.parse_loop_block()? {
            Some(loop_block)
        } else if let Some(while_loop) = self.parse_while_loop()? {
            Some(while_loop)
        } else if let Some(until_loop) = self.parse_until_loop()? {
            Some(until_loop)
        } else if let Some(export_id) = self.parse_export_id(&mut ExpressionContext::line_start())?
        {
            Some(export_id)
        } else if let Some(debug_expression) = self.parse_debug_expression()? {
            Some(debug_expression)
        } else if let Some(result) =
            self.parse_expressions(&mut ExpressionContext::line_start(), false)?
        {
            Some(result)
        } else {
            None
        };

        self.frame_mut()?.finish_expressions();

        Ok(result)
    }

    fn parse_expressions(
        &mut self,
        context: &mut ExpressionContext,
        temp_result: bool,
    ) -> Result<Option<AstIndex>, ParserError> {
        let current_indent = self.lexer.current_indent();

        if context.allow_initial_indentation
            && self.peek_next_token_on_same_line() == Some(Token::NewLineIndented)
        {
            self.consume_until_next_token(context);

            let indent = self.lexer.current_indent();
            if indent <= current_indent {
                return Ok(None);
            }

            if let Some(map_block) = self.parse_map_block(context)? {
                return Ok(Some(map_block));
            }
        }

        let mut expression_context = ExpressionContext {
            allow_space_separated_call: true,
            ..*context
        };

        if let Some(first) = self.parse_expression(&mut expression_context)? {
            let mut expressions = vec![first];
            let mut encountered_comma = false;
            while let Some(Token::Comma) = self.peek_next_token_on_same_line() {
                self.consume_next_token_on_same_line();
                encountered_comma = true;

                if self.peek_next_token(context).is_none() {
                    break;
                }
                self.consume_until_next_token(context);

                if let Some(next_expression) =
                    self.parse_expression_with_lhs(Some(&expressions), &mut expression_context)?
                {
                    match self.ast.node(next_expression).node {
                        Node::Assign { .. }
                        | Node::MultiAssign { .. }
                        | Node::For(_)
                        | Node::While { .. }
                        | Node::Until { .. } => {
                            // These nodes will have consumed the parsed expressions,
                            // so there's no further work to do.
                            // e.g.
                            //   x, y for x, y in a, b
                            //   a, b = c, d
                            //   a, b, c = x
                            return Ok(Some(next_expression));
                        }
                        _ => {}
                    }

                    expressions.push(next_expression);
                }
            }
            if expressions.len() == 1 && !encountered_comma {
                Ok(Some(first))
            } else {
                let result = if temp_result {
                    Node::TempTuple(expressions)
                } else {
                    Node::Tuple(expressions)
                };
                Ok(Some(self.push_node(result)?))
            }
        } else {
            Ok(None)
        }
    }

    fn parse_expression_with_lhs(
        &mut self,
        lhs: Option<&[AstIndex]>,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression_start(lhs, 0, context)
    }

    fn parse_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression_start(None, 0, context)
    }

    fn parse_expression_start(
        &mut self,
        lhs: Option<&[AstIndex]>,
        min_precedence: u8,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let start_line = self.lexer.line_number();

        let expression_start = match self.parse_term(context)? {
            Some(term) => term,
            None => return Ok(None),
        };

        let continue_expression = start_line == self.lexer.line_number();

        if continue_expression {
            if let Some(lhs) = lhs {
                let mut lhs_with_expression_start = lhs.to_vec();
                lhs_with_expression_start.push(expression_start);
                self.parse_expression_continued(&lhs_with_expression_start, min_precedence, context)
            } else {
                self.parse_expression_continued(&[expression_start], min_precedence, context)
            }
        } else {
            Ok(Some(expression_start))
        }
    }

    fn parse_expression_continued(
        &mut self,
        lhs: &[AstIndex],
        min_precedence: u8,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        let last_lhs = match lhs {
            [last] => *last,
            [.., last] => *last,
            _ => return internal_error!(MissingContinuedExpressionLhs, self),
        };

        if let Some((next, peek_count)) = self.peek_next_token(context) {
            match next {
                Assign => return self.parse_assign_expression(lhs, AssignOp::Equal),
                AssignAdd => return self.parse_assign_expression(lhs, AssignOp::Add),
                AssignSubtract => return self.parse_assign_expression(lhs, AssignOp::Subtract),
                AssignMultiply => return self.parse_assign_expression(lhs, AssignOp::Multiply),
                AssignDivide => return self.parse_assign_expression(lhs, AssignOp::Divide),
                AssignModulo => return self.parse_assign_expression(lhs, AssignOp::Modulo),
                _ => {
                    if let Some((left_priority, right_priority)) = operator_precedence(next) {
                        if let Some(token_after_op) = self.peek_token_n(peek_count + 1) {
                            if token_is_whitespace(token_after_op)
                                && left_priority >= min_precedence
                            {
                                let op = self.consume_next_token(context).unwrap();

                                // Move on to the token after the operator
                                if self.peek_next_token(context).is_none() {
                                    return indentation_error!(ExpectedRhsExpression, self);
                                }
                                self.consume_until_next_token(context);

                                let rhs = if let Some(map_block) =
                                    self.parse_map_block(&mut ExpressionContext::permissive())?
                                {
                                    map_block
                                } else if let Some(rhs_expression) =
                                    self.parse_expression_start(None, right_priority, context)?
                                {
                                    rhs_expression
                                } else {
                                    return indentation_error!(ExpectedRhsExpression, self);
                                };

                                let op_node = self.push_ast_op(op, last_lhs, rhs)?;
                                return self.parse_expression_continued(
                                    &[op_node],
                                    min_precedence,
                                    context,
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(Some(last_lhs))
    }

    fn parse_assign_expression(
        &mut self,
        lhs: &[AstIndex],
        assign_op: AssignOp,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.consume_next_token_on_same_line();

        let mut targets = Vec::new();

        for lhs_expression in lhs.iter() {
            match self.ast.node(*lhs_expression).node.clone() {
                Node::Id(id_index) => {
                    if matches!(assign_op, AssignOp::Equal) {
                        self.frame_mut()?
                            .decrement_expression_access_for_id(id_index)
                            .map_err(|_| make_internal_error!(UnexpectedIdInExpression, self))?;

                        self.frame_mut()?.ids_assigned_in_scope.insert(id_index);
                    }
                }
                Node::Lookup(_) | Node::Wildcard => {}
                _ => return syntax_error!(ExpectedAssignmentTarget, self),
            }

            targets.push(AssignTarget {
                target_index: *lhs_expression,
                scope: Scope::Local,
            });
        }

        if targets.is_empty() {
            return internal_error!(MissingAssignmentTarget, self);
        }

        let single_target = targets.len() == 1;
        if let Some(rhs) =
            self.parse_expressions(&mut ExpressionContext::permissive(), !single_target)?
        {
            let node = if single_target {
                Node::Assign {
                    target: *targets.first().unwrap(),
                    op: assign_op,
                    expression: rhs,
                }
            } else {
                Node::MultiAssign {
                    targets,
                    expressions: rhs,
                }
            };
            Ok(Some(self.push_node(node)?))
        } else {
            indentation_error!(ExpectedRhsExpression, self)
        }
    }

    fn parse_id(&mut self, context: &mut ExpressionContext) -> Option<ConstantIndex> {
        match self.peek_next_token(context) {
            Some((Token::Id, _)) => {
                self.consume_next_token(context);
                Some(self.constants.add_string(self.lexer.slice()) as ConstantIndex)
            }
            _ => None,
        }
    }

    fn parse_id_or_wildcard(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Option<ConstantIndexOrWildcard> {
        match self.peek_next_token(context) {
            Some((Token::Id, _)) => {
                self.consume_next_token(context);
                Some(ConstantIndexOrWildcard::Index(
                    self.constants.add_string(self.lexer.slice()) as ConstantIndex,
                ))
            }
            Some((Token::Wildcard, _)) => {
                self.consume_next_token_on_same_line();
                Some(ConstantIndexOrWildcard::Wildcard)
            }
            _ => None,
        }
    }

    fn parse_id_or_string(&mut self) -> Result<Option<ConstantIndex>, ParserError> {
        let result = match self.peek_next_token_on_same_line() {
            Some(Token::Id) => {
                self.consume_next_token_on_same_line();
                Some(self.constants.add_string(self.lexer.slice()) as ConstantIndex)
            }
            Some(Token::String) => {
                self.consume_next_token_on_same_line();
                let s = self.parse_string(self.lexer.slice())?;
                Some(self.constants.add_string(&s) as ConstantIndex)
            }
            _ => None,
        };
        Ok(result)
    }

    fn parse_space_separated_call_args(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Vec<AstIndex>, ParserError> {
        let start_line = self.lexer.line_number();
        let mut args = Vec::new();

        while let Some((_, peek_count)) = self.peek_next_token(&context) {
            let peeked_line = self.lexer.peek_line_number(peek_count);
            if peeked_line > start_line {
                self.consume_until_next_token(context);
            } else if self.peek_token() == Some(Token::Whitespace) {
                self.consume_until_next_token_on_same_line();
            } else {
                break;
            }

            let mut arg_context = ExpressionContext {
                allow_space_separated_call: false,
                allow_linebreaks: true,
                allow_initial_indentation: false,
                expected_indentation: None,
            };

            if let Some(expression) = self.parse_expression(&mut arg_context)? {
                args.push(expression);
            } else {
                break;
            }
        }

        Ok(args)
    }

    fn parse_id_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if let Some(constant_index) = self.parse_id(context) {
            self.frame_mut()?
                .increment_expression_access_for_id(constant_index);

            let id_index = self.push_node(Node::Id(constant_index))?;
            let result = match self.peek_token() {
                Some(Token::Whitespace) if context.allow_space_separated_call => {
                    let start_span = self.lexer.span();
                    let args = self.parse_space_separated_call_args(context)?;

                    if args.is_empty() {
                        id_index
                    } else {
                        self.push_node_with_start_span(
                            Node::Call {
                                function: id_index,
                                args,
                            },
                            start_span,
                        )?
                    }
                }
                Some(_) if self.next_token_is_lookup_start(context) => {
                    self.parse_lookup(id_index, context)?
                }
                Some(_) if context.allow_space_separated_call && context.allow_linebreaks => {
                    let start_span = self.lexer.span();
                    let args = self.parse_space_separated_call_args(context)?;

                    if args.is_empty() {
                        id_index
                    } else {
                        self.push_node_with_start_span(
                            Node::Call {
                                function: id_index,
                                args,
                            },
                            start_span,
                        )?
                    }
                }
                _ => id_index,
            };

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn parse_lookup(
        &mut self,
        root: AstIndex,
        context: &mut ExpressionContext,
    ) -> Result<AstIndex, ParserError> {
        let mut lookup = Vec::new();

        let start_indent = self.lexer.current_indent();
        let mut lookup_indent = None;
        let mut node_context = ExpressionContext {
            expected_indentation: None,
            ..*context
        };
        let mut node_start_span = self.lexer.span();

        lookup.push((LookupNode::Root(root), node_start_span));

        while let Some(token) = self.peek_token() {
            if let Some(lookup_indent) = lookup_indent {
                if self.lexer.current_indent() != lookup_indent {
                    return syntax_error!(UnexpectedIndentation, self);
                }
            }

            match token {
                Token::ParenOpen => {
                    node_start_span = self.lexer.span();
                    let args = self.parse_parenthesized_args()?;
                    lookup.push((
                        LookupNode::Call(args),
                        self.span_with_start(node_start_span),
                    ));
                }
                Token::ListStart => {
                    self.consume_token();
                    node_start_span = self.lexer.span();

                    let mut index_context = ExpressionContext::restricted();

                    let index_expression = if let Some(index_expression) =
                        self.parse_expression(&mut index_context)?
                    {
                        match self.peek_token() {
                            Some(Token::Range) => {
                                self.consume_token();

                                if let Some(end_expression) =
                                    self.parse_expression(&mut index_context)?
                                {
                                    self.push_node(Node::Range {
                                        start: index_expression,
                                        end: end_expression,
                                        inclusive: false,
                                    })?
                                } else {
                                    self.push_node(Node::RangeFrom {
                                        start: index_expression,
                                    })?
                                }
                            }
                            Some(Token::RangeInclusive) => {
                                self.consume_token();

                                if let Some(end_expression) =
                                    self.parse_expression(&mut index_context)?
                                {
                                    self.push_node(Node::Range {
                                        start: index_expression,
                                        end: end_expression,
                                        inclusive: true,
                                    })?
                                } else {
                                    self.push_node(Node::RangeFrom {
                                        start: index_expression,
                                    })?
                                }
                            }
                            _ => index_expression,
                        }
                    } else {
                        // Look for RangeTo/RangeFull
                        // e.g. x[..10], y[..]
                        match self.peek_next_token_on_same_line() {
                            Some(Token::Range) => {
                                self.consume_next_token_on_same_line();

                                if let Some(end_expression) =
                                    self.parse_expression(&mut index_context)?
                                {
                                    self.push_node(Node::RangeTo {
                                        end: end_expression,
                                        inclusive: false,
                                    })?
                                } else {
                                    self.push_node(Node::RangeFull)?
                                }
                            }
                            Some(Token::RangeInclusive) => {
                                self.consume_next_token_on_same_line();

                                if let Some(end_expression) =
                                    self.parse_expression(&mut index_context)?
                                {
                                    self.push_node(Node::RangeTo {
                                        end: end_expression,
                                        inclusive: true,
                                    })?
                                } else {
                                    self.push_node(Node::RangeFull)?
                                }
                            }
                            _ => return syntax_error!(ExpectedIndexExpression, self),
                        }
                    };

                    if let Some(Token::ListEnd) = self.peek_next_token_on_same_line() {
                        self.consume_next_token_on_same_line();
                        lookup.push((
                            LookupNode::Index(index_expression),
                            self.span_with_start(node_start_span),
                        ));
                    } else {
                        return syntax_error!(ExpectedIndexEnd, self);
                    }
                }
                Token::Dot => {
                    self.consume_token();

                    if !matches!(self.peek_token(), Some(Token::Id) | Some(Token::String)) {
                        return syntax_error!(ExpectedMapKey, self);
                    } else if let Some(id_index) = self.parse_id_or_string()? {
                        lookup.push((
                            LookupNode::Id(id_index),
                            self.span_with_start(self.lexer.span()),
                        ));
                    } else {
                        return syntax_error!(ExpectedMapKey, self);
                    }
                }
                Token::Whitespace if node_context.allow_space_separated_call => {
                    let args = self.parse_space_separated_call_args(context)?;

                    if args.is_empty() {
                        break;
                    } else {
                        lookup.push((LookupNode::Call(args), node_start_span));

                        node_context = ExpressionContext {
                            allow_space_separated_call: false,
                            ..node_context
                        };
                    }
                }
                _ if matches!(self.peek_next_token(&node_context), Some((Token::Dot, _))) => {
                    self.consume_until_next_token(&mut node_context);
                    let new_indent = self.lexer.current_indent();

                    if lookup_indent.is_none() {
                        if new_indent > start_indent {
                            lookup_indent = Some(new_indent);
                        } else {
                            break;
                        }
                    }

                    node_context = ExpressionContext {
                        allow_space_separated_call: true,
                        ..node_context
                    };
                }
                _ => break,
            }
        }

        // Add the lookup nodes to the AST in reverse order:
        // the final AST index will be the lookup root node.
        let mut next_index = None;
        for (node, span) in lookup.iter().rev() {
            next_index =
                Some(self.push_node_with_span(Node::Lookup((node.clone(), next_index)), *span)?);
        }
        next_index.ok_or_else(|| make_internal_error!(LookupParseFailure, self))
    }

    fn parse_parenthesized_args(&mut self) -> Result<Vec<AstIndex>, ParserError> {
        if self.consume_next_token_on_same_line() != Some(Token::ParenOpen) {
            return internal_error!(ArgumentsParseFailure, self);
        }

        let start_indent = self.lexer.current_indent();

        let mut args = Vec::new();

        let mut args_context = ExpressionContext::permissive();

        while self.peek_next_token(&args_context).is_some() {
            self.consume_until_next_token(&mut args_context);

            if let Some(expression) = self.parse_expression(&mut ExpressionContext::restricted())? {
                args.push(expression);
            } else {
                break;
            }

            if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                self.consume_next_token_on_same_line();
            } else {
                break;
            }
        }

        let mut args_end_context = ExpressionContext::permissive();
        args_end_context.expected_indentation = Some(start_indent);
        if !matches!(
            self.peek_next_token(&args_end_context),
            Some((Token::ParenClose, _))
        ) {
            return syntax_error!(ExpectedArgsEnd, self);
        }

        self.consume_next_token(&mut args_end_context);
        Ok(args)
    }

    fn parse_range(&mut self, lhs: Option<AstIndex>) -> Result<Option<AstIndex>, ParserError> {
        use Node::{Range, RangeFrom, RangeFull, RangeTo};

        let inclusive = match self.peek_token() {
            Some(Token::Range) => false,
            Some(Token::RangeInclusive) => true,
            _ => return Ok(None),
        };

        self.consume_token();

        let rhs = self.parse_term(&mut ExpressionContext::restricted())?;

        let node = match (lhs, rhs) {
            (Some(start), Some(end)) => Range {
                start,
                end,
                inclusive,
            },
            (Some(start), None) => RangeFrom { start },
            (None, Some(end)) => RangeTo { end, inclusive },
            (None, None) => RangeFull,
        };

        Ok(Some(self.push_node(node)?))
    }

    fn parse_export_id(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() == Some(Token::Export) {
            self.consume_next_token_on_same_line();

            if let Some(constant_index) = self.parse_id(context) {
                let export_id = self.push_node(Node::Id(constant_index))?;

                match self.peek_next_token_on_same_line() {
                    Some(Token::Assign) => {
                        self.consume_next_token_on_same_line();

                        if let Some(rhs) =
                            self.parse_expressions(&mut ExpressionContext::permissive(), false)?
                        {
                            let node = Node::Assign {
                                target: AssignTarget {
                                    target_index: export_id,
                                    scope: Scope::Global,
                                },
                                op: AssignOp::Equal,
                                expression: rhs,
                            };

                            Ok(Some(self.push_node(node)?))
                        } else {
                            return indentation_error!(ExpectedRhsExpression, self);
                        }
                    }
                    Some(Token::NewLine) | Some(Token::NewLineIndented) => Ok(Some(export_id)),
                    _ => syntax_error!(UnexpectedTokenAfterExportId, self),
                }
            } else {
                syntax_error!(ExpectedExportExpression, self)
            }
        } else {
            Ok(None)
        }
    }

    fn parse_debug_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() != Some(Token::Debug) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();

        let start_position = self.lexer.span().start;

        self.consume_until_next_token_on_same_line();

        let mut context = ExpressionContext::permissive();
        let expression_source_start = self.lexer.source_position();
        let expression = if let Some(expression) = self.parse_expressions(&mut context, false)? {
            expression
        } else {
            return syntax_error!(ExpectedExpression, self);
        };

        let expression_source_end = self.lexer.source_position();

        let expression_string = self
            .constants
            .add_string(&self.lexer.source()[expression_source_start..expression_source_end])
            as u32;

        let result = self.ast.push(
            Node::Debug {
                expression_string,
                expression,
            },
            Span {
                start: start_position,
                end: self.lexer.span().end,
            },
        )?;

        Ok(Some(result))
    }

    fn parse_term(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Node::*;

        // let current_indent = self.lexer.current_indent();

        if let Some((token, peek_count)) = self.peek_next_token(context) {
            let result = match token {
                Token::True => {
                    self.consume_next_token(context);
                    Some(self.push_node(BoolTrue)?)
                }
                Token::False => {
                    self.consume_next_token(context);
                    Some(self.push_node(BoolFalse)?)
                }
                Token::ParenOpen => self.parse_nested_expressions(context)?,
                Token::Number => {
                    self.consume_next_token(context);
                    let number_node = match f64::from_str(self.lexer.slice()) {
                        Ok(n) => {
                            if f64_eq(n, 0.0) {
                                self.push_node(Number0)?
                            } else if f64_eq(n, 1.0) {
                                self.push_node(Number1)?
                            } else {
                                let constant_index = self.constants.add_number(n) as u32;
                                self.push_node(Number(constant_index))?
                            }
                        }
                        Err(_) => {
                            return internal_error!(NumberParseFailure, self);
                        }
                    };
                    if self.next_token_is_lookup_start(context) {
                        Some(self.parse_lookup(number_node, context)?)
                    } else {
                        Some(number_node)
                    }
                }
                Token::String => {
                    self.consume_next_token(context);
                    let s = self.parse_string(self.lexer.slice())?;
                    let constant_index = self.constants.add_string(&s) as u32;
                    let string_node = self.push_node(Str(constant_index))?;
                    if self.next_token_is_lookup_start(context) {
                        Some(self.parse_lookup(string_node, context)?)
                    } else {
                        Some(string_node)
                    }
                }
                Token::Id => self.parse_id_expression(context)?,
                Token::Wildcard => {
                    self.consume_next_token(context);
                    Some(self.push_node(Node::Wildcard)?)
                }
                Token::ListStart => self.parse_list(context)?,
                Token::MapStart => self.parse_map_inline(context)?,
                Token::Num2 => {
                    self.consume_next_token(context);
                    let start_span = self.lexer.span();

                    let args = if self.peek_token() == Some(Token::ParenOpen) {
                        self.parse_parenthesized_args()?
                    } else {
                        self.parse_space_separated_call_args(&mut ExpressionContext::permissive())?
                    };

                    if args.is_empty() {
                        return syntax_error!(ExpectedExpression, self);
                    } else if args.len() > 2 {
                        return syntax_error!(TooManyNum2Terms, self);
                    }

                    Some(self.push_node_with_start_span(Num2(args), start_span)?)
                }
                Token::Num4 => {
                    self.consume_next_token(context);
                    let start_span = self.lexer.span();

                    let args = if self.peek_token() == Some(Token::ParenOpen) {
                        self.parse_parenthesized_args()?
                    } else {
                        self.parse_space_separated_call_args(&mut ExpressionContext::permissive())?
                    };

                    if args.is_empty() {
                        return syntax_error!(ExpectedExpression, self);
                    } else if args.len() > 4 {
                        return syntax_error!(TooManyNum4Terms, self);
                    }

                    Some(self.push_node_with_start_span(Num4(args), start_span)?)
                }
                Token::If if context.allow_space_separated_call => {
                    self.parse_if_expression(context)?
                }
                Token::Match => self.parse_match_expression(context)?,
                Token::Function => self.parse_function(context)?,
                Token::Copy => {
                    self.consume_next_token(context);
                    if let Some(expression) = self.parse_expression(&mut ExpressionContext {
                        allow_space_separated_call: true,
                        expected_indentation: None,
                        ..*context
                    })? {
                        Some(self.push_node(Node::CopyExpression(expression))?)
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Subtract => {
                    if let Some(token_after_subtract) = self.peek_token_n(peek_count + 1) {
                        if !token_is_whitespace(token_after_subtract) {
                            self.consume_next_token(context);
                            if let Some(term) =
                                self.parse_term(&mut ExpressionContext::restricted())?
                            {
                                Some(self.push_node(Node::Negate(term))?)
                            } else {
                                return syntax_error!(ExpectedExpression, self);
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                Token::Not => {
                    self.consume_next_token(context);
                    if let Some(expression) = self.parse_expression(&mut ExpressionContext {
                        allow_space_separated_call: true,
                        expected_indentation: None,
                        ..*context
                    })? {
                        Some(self.push_node(Node::Negate(expression))?)
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Type => {
                    self.consume_next_token(context);
                    if let Some(expression) = self.parse_expression(&mut ExpressionContext {
                        allow_space_separated_call: true,
                        expected_indentation: None,
                        ..*context
                    })? {
                        Some(self.push_node(Node::Type(expression))?)
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Yield => {
                    self.consume_next_token(context);
                    if let Some(expression) = self.parse_expressions(
                        &mut ExpressionContext {
                            allow_space_separated_call: true,
                            expected_indentation: None,
                            ..*context
                        },
                        false,
                    )? {
                        let result = self.push_node(Node::Yield(expression))?;
                        self.frame_mut()?.contains_yield = true;
                        Some(result)
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Break => {
                    self.consume_next_token(context);
                    Some(self.push_node(Node::Break)?)
                }
                Token::Continue => {
                    self.consume_next_token(context);
                    Some(self.push_node(Node::Continue)?)
                }
                Token::Return => {
                    self.consume_next_token(context);
                    let result = if let Some(expression) = self.parse_expressions(
                        &mut ExpressionContext {
                            allow_space_separated_call: true,
                            expected_indentation: None,
                            ..*context
                        },
                        false,
                    )? {
                        self.push_node(Node::ReturnExpression(expression))?
                    } else {
                        self.push_node(Node::Return)?
                    };
                    Some(result)
                }
                Token::From | Token::Import => self.parse_import_expression(context)?,
                Token::Try if context.allow_space_separated_call => {
                    self.parse_try_expression(context)?
                }
                // Token::NewLineIndented => self.parse_map_block(current_indent, None)?,
                Token::Error => return syntax_error!(LexerError, self),
                _ => None,
            };

            let result = match self.peek_token() {
                Some(Token::Range) | Some(Token::RangeInclusive) => self.parse_range(result)?,
                _ => result,
            };

            Ok(result)
        } else {
            Ok(None)
        }
    }

    fn parse_list(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let mut list_context = *context;
        let start_indent = self.lexer.current_indent();
        let start_span = self.lexer.span();

        if self.consume_next_token(&mut list_context) != Some(Token::ListStart) {
            return internal_error!(UnexpectedToken, self);
        }

        // The end brace should have the same indentation as the start brace.
        if list_context.expected_indentation.is_none() {
            list_context.expected_indentation = Some(start_indent);
        }

        let mut entries = Vec::new();

        let mut entry_context = ExpressionContext::permissive();
        while !matches!(
            self.peek_next_token(&entry_context),
            Some((Token::ListEnd, _)) | None
        ) {
            self.consume_until_next_token(&mut entry_context);

            if let Some(entry) = self.parse_expression(&mut ExpressionContext::inline())? {
                entries.push(entry);
            }

            if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                self.consume_next_token_on_same_line();
            } else {
                break;
            }
        }

        // Consume the list end
        if !matches!(
            self.peek_next_token(&list_context),
            Some((Token::ListEnd, _))
        ) {
            return syntax_error!(ExpectedListEnd, self);
        }
        self.consume_next_token(&mut list_context);

        let list_node = self.push_node_with_start_span(Node::List(entries), start_span)?;

        let result = if self.next_token_is_lookup_start(&mut list_context) {
            self.parse_lookup(list_node, &mut list_context)?
        } else {
            list_node
        };

        Ok(Some(result))
    }

    fn parse_indented_map_or_block(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let mut context = ExpressionContext::permissive();

        let start_indent = self.lexer.current_indent();

        if let Some((_, peek_count)) = self.peek_next_token(&context) {
            if self.lexer.peek_indent(peek_count) > start_indent {
                let result = if let Some(map_block) = self.parse_map_block(&mut context)? {
                    Some(map_block)
                } else if let Some(block) = self.parse_indented_block(&mut context)? {
                    Some(block)
                } else {
                    None
                };

                return Ok(result);
            }
        }

        Ok(None)
    }

    fn parse_map_block(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if let Some((_, peek_count)) = self.peek_next_token(context) {
            // The first entry in a map block should have a defined value
            if self.peek_token_n(peek_count + 1) != Some(Token::Colon) {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        self.consume_until_next_token(context);
        let start_span = self.lexer.span();

        let mut entries = Vec::new();

        while let Some(key) = self.parse_id_or_string()? {
            if self.peek_next_token_on_same_line() == Some(Token::Colon) {
                self.consume_next_token_on_same_line();

                if let Some(value) =
                    self.parse_expressions(&mut ExpressionContext::inline(), false)?
                {
                    entries.push((key, Some(value)));
                } else {
                    // If a value wasn't found on the same line as the key,
                    // look for an indented value
                    if let Some(value) = self.parse_indented_map_or_block()? {
                        entries.push((key, Some(value)));
                    } else {
                        return syntax_error!(ExpectedMapValue, self);
                    }
                }
            } else {
                entries.push((key, None));
            }

            // self.consume_until_next_token(context);
            if self.peek_next_token(context).is_none() {
                break;
            }

            self.consume_until_next_token(context);
        }

        if entries.is_empty() {
            return Ok(None);
        }

        Ok(Some(self.push_node_with_start_span(
            Node::Map(entries),
            start_span,
        )?))
    }

    fn parse_map_inline(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.consume_next_token(context) != Some(Token::MapStart) {
            return internal_error!(UnexpectedToken, self);
        }

        let start_indent = self.lexer.current_indent();
        let start_span = self.lexer.span();

        let mut entries = Vec::new();

        while self.peek_next_token(context).is_some() {
            self.consume_until_next_token(context);

            if let Some(key) = self.parse_id_or_string()? {
                if self.peek_token() == Some(Token::Colon) {
                    self.consume_token();

                    let mut value_context = ExpressionContext::permissive();
                    if self.peek_next_token(&value_context).is_none() {
                        return syntax_error!(ExpectedMapValue, self);
                    }
                    self.consume_until_next_token(&mut value_context);

                    if let Some(value) = self.parse_expression(&mut value_context)? {
                        entries.push((key, Some(value)));
                    } else {
                        return syntax_error!(ExpectedMapValue, self);
                    }
                } else {
                    entries.push((key, None));
                }

                if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                    self.consume_next_token_on_same_line();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let mut map_end_context = ExpressionContext::permissive();
        map_end_context.expected_indentation = Some(start_indent);
        if !matches!(
            self.peek_next_token(&map_end_context),
            Some((Token::MapEnd, _))
        ) {
            return syntax_error!(ExpectedMapEnd, self);
        }
        self.consume_next_token(&mut map_end_context);

        let map_node = self.push_node_with_start_span(Node::Map(entries), start_span)?;

        let result = if self.next_token_is_lookup_start(context) {
            self.parse_lookup(map_node, context)?
        } else {
            map_node
        };

        Ok(Some(result))
    }

    fn parse_for_loop(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() != Some(Token::For) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();

        let start_span = self.lexer.span();

        let mut args = Vec::new();
        while let Some(id_or_wildcard) = self.parse_id_or_wildcard(context) {
            match id_or_wildcard {
                ConstantIndexOrWildcard::Index(id_index) => {
                    args.push(Some(id_index));
                    self.frame_mut()?.ids_assigned_in_scope.insert(id_index);
                }
                ConstantIndexOrWildcard::Wildcard => args.push(None),
            }

            match self.peek_next_token_on_same_line() {
                Some(Token::Comma) => {
                    self.consume_next_token_on_same_line();
                }
                Some(Token::In) => {
                    self.consume_next_token_on_same_line();
                    break;
                }
                _ => return syntax_error!(ExpectedForInKeyword, self),
            }
        }
        if args.is_empty() {
            return syntax_error!(ExpectedForArgs, self);
        }

        let range = match self.parse_expression(&mut ExpressionContext::inline())? {
            Some(range) => range,
            None => return syntax_error!(ExpectedForRanges, self),
        };

        match self.parse_indented_block(&mut ExpressionContext::permissive())? {
            Some(body) => {
                let result = self.push_node_with_start_span(
                    Node::For(AstFor { args, range, body }),
                    start_span,
                )?;

                Ok(Some(result))
            }
            None => indentation_error!(ExpectedForBody, self),
        }
    }

    fn parse_loop_block(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() != Some(Token::Loop) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();

        if let Some(body) = self.parse_indented_block(&mut ExpressionContext::permissive())? {
            let result = self.push_node(Node::Loop { body })?;
            Ok(Some(result))
        } else {
            return indentation_error!(ExpectedLoopBody, self);
        }
    }

    fn parse_while_loop(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() != Some(Token::While) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();

        let condition =
            if let Some(condition) = self.parse_expression(&mut ExpressionContext::inline())? {
                condition
            } else {
                return syntax_error!(ExpectedWhileCondition, self);
            };

        match self.parse_indented_block(&mut ExpressionContext::permissive())? {
            Some(body) => {
                let result = self.push_node(Node::While { condition, body })?;
                Ok(Some(result))
            }
            None => indentation_error!(ExpectedWhileBody, self),
        }
    }

    fn parse_until_loop(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_next_token_on_same_line() != Some(Token::Until) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();

        let condition =
            if let Some(condition) = self.parse_expression(&mut ExpressionContext::inline())? {
                condition
            } else {
                return syntax_error!(ExpectedUntilCondition, self);
            };

        match self.parse_indented_block(&mut ExpressionContext::permissive())? {
            Some(body) => {
                let result = self.push_node(Node::Until { condition, body })?;
                Ok(Some(result))
            }
            None => indentation_error!(ExpectedUntilBody, self),
        }
    }

    fn parse_if_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let current_indent = self.lexer.current_indent();
        context.expected_indentation = Some(current_indent);

        if self.consume_next_token(context) != Some(Token::If) {
            return internal_error!(UnexpectedToken, self);
        }

        let condition = match self.parse_expression(&mut ExpressionContext::inline())? {
            Some(condition) => condition,
            None => return syntax_error!(ExpectedIfCondition, self),
        };

        let result = if self.peek_next_token_on_same_line() == Some(Token::Then) {
            self.consume_next_token_on_same_line();
            let then_node = match self.parse_expressions(&mut ExpressionContext::inline(), true)? {
                Some(then_node) => then_node,
                None => return syntax_error!(ExpectedThenExpression, self),
            };
            let else_node = if self.peek_next_token_on_same_line() == Some(Token::Else) {
                self.consume_next_token_on_same_line();
                match self.parse_expressions(&mut ExpressionContext::inline(), true)? {
                    Some(else_node) => Some(else_node),
                    None => return syntax_error!(ExpectedElseExpression, self),
                }
            } else {
                None
            };

            self.push_node(Node::If(AstIf {
                condition,
                then_node,
                else_if_blocks: vec![],
                else_node,
            }))?
        } else if let Some(then_node) = self.parse_indented_map_or_block()? {
            let mut else_if_blocks = Vec::new();

            while let Some((Token::ElseIf, _)) = self.peek_next_token(context) {
                self.consume_next_token(context);
                if let Some(else_if_condition) =
                    self.parse_expression(&mut ExpressionContext::inline())?
                {
                    if let Some(else_if_block) = self.parse_indented_map_or_block()? {
                        else_if_blocks.push((else_if_condition, else_if_block));
                    } else {
                        return indentation_error!(ExpectedElseIfBlock, self);
                    }
                } else {
                    return syntax_error!(ExpectedElseIfCondition, self);
                }
            }

            let else_node = if let Some((Token::Else, _)) = self.peek_next_token(context) {
                self.consume_next_token(context);
                if let Some(else_block) = self.parse_indented_map_or_block()? {
                    Some(else_block)
                } else {
                    return indentation_error!(ExpectedElseBlock, self);
                }
            } else {
                None
            };

            self.push_node(Node::If(AstIf {
                condition,
                then_node,
                else_if_blocks,
                else_node,
            }))?
        } else {
            return indentation_error!(ExpectedThenKeywordOrBlock, self);
        };

        Ok(Some(result))
    }

    fn parse_match_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.consume_next_token(context) != Some(Token::Match) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();
        let start_span = self.lexer.span();

        let expression = match self.parse_expressions(&mut ExpressionContext::inline(), true)? {
            Some(expression) => expression,
            None => return syntax_error!(ExpectedMatchExpression, self),
        };

        self.consume_until_next_token(context);

        let match_indent = self.lexer.current_indent();
        if match_indent <= current_indent {
            return indentation_error!(ExpectedMatchArm, self);
        }

        let mut arms = Vec::new();

        while self.peek_token().is_some() {
            // Match patterns for a single arm, with alternatives separated by 'or'
            // e.g. match x, y
            //   0, 1 then ...
            //   2, 3 or 4, 5 then ...
            //   other then ...
            let mut arm_patterns = Vec::new();
            let mut expected_arm_count = 1;

            while let Some(pattern) = self.parse_match_pattern()? {
                // Match patterns, separated by commas in the case of matching multi-expressions
                let mut patterns = vec![pattern];

                while let Some(Token::Comma) = self.peek_next_token_on_same_line() {
                    self.consume_next_token_on_same_line();

                    match self.parse_match_pattern()? {
                        Some(pattern) => patterns.push(pattern),
                        None => return syntax_error!(ExpectedMatchPattern, self),
                    }
                }

                arm_patterns.push(match patterns.as_slice() {
                    [single_pattern] => *single_pattern,
                    _ => self.push_node(Node::TempTuple(patterns))?,
                });

                if let Some(Token::Or) = self.peek_next_token_on_same_line() {
                    self.consume_next_token_on_same_line();
                    expected_arm_count += 1;
                }
            }

            if arm_patterns.len() != expected_arm_count {
                return syntax_error!(ExpectedMatchPattern, self);
            }

            let condition = if self.peek_next_token_on_same_line() == Some(Token::If) {
                self.consume_next_token_on_same_line();
                match self.parse_expression(&mut ExpressionContext::inline())? {
                    Some(expression) => Some(expression),
                    None => return syntax_error!(ExpectedMatchCondition, self),
                }
            } else {
                None
            };

            let expression = if self.peek_next_token_on_same_line() == Some(Token::Then) {
                self.consume_next_token_on_same_line();
                match self.parse_expressions(&mut ExpressionContext::inline(), true)? {
                    Some(expression) => expression,
                    None => return syntax_error!(ExpectedMatchArmExpressionAfterThen, self),
                }
            } else if let Some(indented_expression) = self.parse_indented_map_or_block()? {
                indented_expression
            } else {
                return syntax_error!(ExpectedMatchArmExpression, self);
            };

            arms.push(MatchArm {
                patterns: arm_patterns,
                condition,
                expression,
            });

            if self.peek_next_token(context).is_none() {
                break;
            }

            self.consume_until_next_token(context);
        }

        Ok(Some(self.push_node_with_start_span(
            Node::Match { expression, arms },
            start_span,
        )?))
    }

    fn parse_match_pattern(&mut self) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        let mut pattern_context = ExpressionContext::restricted();

        let result = match self.peek_next_token(&pattern_context) {
            Some((token, _)) => match token {
                True | False | Number | String => return self.parse_term(&mut pattern_context),
                Id => match self.parse_id(&mut pattern_context) {
                    Some(id) => {
                        self.frame_mut()?.ids_assigned_in_scope.insert(id);
                        Some(self.push_node(Node::Id(id))?)
                    }
                    None => return internal_error!(IdParseFailure, self),
                },
                Wildcard => {
                    self.consume_next_token_on_same_line();
                    Some(self.push_node(Node::Wildcard)?)
                }
                ListStart => {
                    self.consume_next_token_on_same_line();

                    let list_patterns = self.parse_nested_match_patterns()?;

                    if self.consume_next_token_on_same_line() != Some(ListEnd) {
                        return syntax_error!(ExpectedListEnd, self);
                    }

                    Some(self.push_node(Node::List(list_patterns))?)
                }
                ParenOpen => {
                    self.consume_next_token_on_same_line();

                    if self.peek_token() == Some(ParenClose) {
                        self.consume_token();
                        Some(self.push_node(Node::Empty)?)
                    } else {
                        let tuple_patterns = self.parse_nested_match_patterns()?;

                        if self.consume_next_token_on_same_line() != Some(ParenClose) {
                            return syntax_error!(ExpectedCloseParen, self);
                        }

                        Some(self.push_node(Node::Tuple(tuple_patterns))?)
                    }
                }
                _ => None,
            },
            None => None,
        };

        Ok(result)
    }

    fn parse_nested_match_patterns(&mut self) -> Result<Vec<AstIndex>, ParserError> {
        let mut result = vec![];

        while let Some(pattern) = self.parse_match_pattern()? {
            result.push(pattern);

            if self.peek_next_token_on_same_line() != Some(Token::Comma) {
                break;
            }
            self.consume_next_token_on_same_line();
        }

        Ok(result)
    }

    fn parse_import_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let from_import = match self.consume_next_token(context) {
            Some(Token::From) => true,
            Some(Token::Import) => false,
            _ => return internal_error!(UnexpectedToken, self),
        };

        let start_span = self.lexer.span();

        let from = if from_import {
            let from = match self.consume_import_items()?.as_slice() {
                [from] => from.clone(),
                _ => return syntax_error!(ImportFromExpressionHasTooManyItems, self),
            };

            if self.peek_next_token_on_same_line() != Some(Token::Import) {
                return syntax_error!(ExpectedImportKeywordAfterFrom, self);
            }
            self.consume_next_token_on_same_line();
            from
        } else {
            vec![]
        };

        let items = self.consume_import_items()?;

        if let Some(token) = self.peek_next_token_on_same_line() {
            if !token_is_whitespace(token) {
                self.consume_next_token_on_same_line();
                return syntax_error!(UnexpectedTokenInImportExpression, self);
            }
        }

        for item in items.iter() {
            match item.last() {
                Some(id) => {
                    self.frame_mut()?.ids_assigned_in_scope.insert(*id);
                }
                None => return internal_error!(ExpectedIdInImportItem, self),
            }
        }

        Ok(Some(self.push_node_with_start_span(
            Node::Import { from, items },
            start_span,
        )?))
    }

    fn parse_try_expression(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.consume_next_token(context) != Some(Token::Try) {
            return internal_error!(UnexpectedToken, self);
        }

        context.expected_indentation = Some(self.lexer.current_indent());

        let start_span = self.lexer.span();

        let try_block = if let Some(try_block) =
            self.parse_indented_block(&mut ExpressionContext::permissive())?
        {
            try_block
        } else {
            return indentation_error!(ExpectedTryBody, self);
        };

        if !matches!(self.peek_next_token(context), Some((Token::Catch, _))) {
            return syntax_error!(ExpectedCatch, self);
        }
        self.consume_next_token(context);

        let catch_arg = if let Some(catch_arg) =
            self.parse_id_or_wildcard(&mut ExpressionContext::restricted())
        {
            match catch_arg {
                ConstantIndexOrWildcard::Index(id_index) => {
                    self.frame_mut()?.ids_assigned_in_scope.insert(id_index);
                    Some(id_index)
                }

                ConstantIndexOrWildcard::Wildcard => None,
            }
        } else {
            return syntax_error!(ExpectedCatchArgument, self);
        };

        let catch_block = if let Some(catch_block) =
            self.parse_indented_block(&mut ExpressionContext::permissive())?
        {
            catch_block
        } else {
            return indentation_error!(ExpectedCatchBody, self);
        };

        let finally_block = if matches!(self.peek_next_token(context), Some((Token::Finally, _))) {
            self.consume_next_token(context);
            if let Some(finally_block) =
                self.parse_indented_block(&mut ExpressionContext::permissive())?
            {
                Some(finally_block)
            } else {
                return indentation_error!(ExpectedFinallyBody, self);
            }
        } else {
            None
        };

        let result = self.push_node_with_start_span(
            Node::Try(AstTry {
                try_block,
                catch_arg,
                catch_block,
                finally_block,
            }),
            start_span,
        )?;

        Ok(Some(result))
    }

    fn consume_import_items(&mut self) -> Result<Vec<Vec<ConstantIndex>>, ParserError> {
        let mut items = vec![];
        let mut item_context = ExpressionContext::permissive();

        while let Some(item_root) = self.parse_id(&mut item_context) {
            let mut item = vec![item_root];

            while self.peek_token() == Some(Token::Dot) {
                self.consume_token();

                match self.parse_id(&mut ExpressionContext::restricted()) {
                    Some(id) => item.push(id),
                    None => return syntax_error!(ExpectedImportModuleId, self),
                }
            }

            items.push(item);

            if self.peek_next_token_on_same_line() != Some(Token::Comma) {
                break;
            }
            self.consume_next_token_on_same_line();
        }

        if items.is_empty() {
            return syntax_error!(ExpectedIdInImportExpression, self);
        }

        Ok(items)
    }

    fn parse_indented_block(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        context.expected_indentation = None;

        if self.peek_next_token(context).is_none() {
            return Ok(None);
        }

        self.consume_until_next_token(context);

        let mut body = Vec::new();

        let start_span = self.lexer.span();

        while let Some(expression) = self.parse_line()? {
            body.push(expression);

            if self.peek_next_token(context).is_none() {
                break;
            }

            self.consume_until_next_token(context);
        }

        // If the body is a single expression then it doesn't need to be wrapped in a block
        if body.len() == 1 {
            Ok(Some(*body.first().unwrap()))
        } else {
            Ok(Some(self.push_node_with_start_span(
                Node::Block(body),
                start_span,
            )?))
        }
    }

    fn parse_nested_expressions(
        &mut self,
        context: &mut ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        if self.consume_next_token(context) != Some(ParenOpen) {
            return internal_error!(UnexpectedToken, self);
        }

        let expression_context = ExpressionContext {
            allow_space_separated_call: true,
            ..*context
        };
        let mut expressions = vec![];
        let mut encountered_comma = false;
        while let Some(expression) = self.parse_expression(&mut expression_context.clone())? {
            expressions.push(expression);

            if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                self.consume_next_token_on_same_line();
                encountered_comma = true;
            } else {
                break;
            }
        }

        let result = match expressions.as_slice() {
            [] => self.push_node(Node::Empty)?,
            [single_expression] if !encountered_comma => *single_expression,
            _ => self.push_node(Node::Tuple(expressions))?,
        };

        if let Some(ParenClose) = self.peek_token() {
            self.consume_token();
            let result = if self.next_token_is_lookup_start(context) {
                self.parse_lookup(result, context)?
            } else {
                result
            };
            Ok(Some(result))
        } else {
            syntax_error!(ExpectedCloseParen, self)
        }
    }

    fn parse_string(&self, s: &str) -> Result<String, ParserError> {
        let without_quotes = &s[1..s.len() - 1];

        let mut result = String::with_capacity(without_quotes.len());
        let mut chars = without_quotes.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('\\') => result.push('\\'),
                    Some('\'') => result.push('\''),
                    Some('"') => result.push('"'),
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('\n') | Some('\r') => {
                        while let Some(c) = chars.peek() {
                            if c.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                    _ => return syntax_error!(UnexpectedEscapeInString, self),
                },
                _ => result.push(c),
            }
        }

        Ok(result)
    }

    fn push_ast_op(
        &mut self,
        op: Token,
        lhs: AstIndex,
        rhs: AstIndex,
    ) -> Result<AstIndex, ParserError> {
        use Token::*;
        let ast_op = match op {
            Add => AstOp::Add,
            Subtract => AstOp::Subtract,
            Multiply => AstOp::Multiply,
            Divide => AstOp::Divide,
            Modulo => AstOp::Modulo,

            Equal => AstOp::Equal,
            NotEqual => AstOp::NotEqual,

            Greater => AstOp::Greater,
            GreaterOrEqual => AstOp::GreaterOrEqual,
            Less => AstOp::Less,
            LessOrEqual => AstOp::LessOrEqual,

            And => AstOp::And,
            Or => AstOp::Or,

            _ => unreachable!(),
        };
        self.push_node(Node::BinaryOp {
            op: ast_op,
            lhs,
            rhs,
        })
    }

    fn next_token_is_lookup_start(&mut self, context: &mut ExpressionContext) -> bool {
        use Token::*;

        if matches!(
            self.peek_token(),
            Some(Dot) | Some(ListStart) | Some(ParenOpen)
        ) {
            return true;
        } else if context.allow_linebreaks {
            let start_line = self.lexer.line_number();
            let start_indent = self.lexer.current_indent();
            if let Some((next_token, peek_count)) = self.peek_next_token(context) {
                let next_line = self.lexer.peek_line_number(peek_count);
                let next_indent = self.lexer.peek_indent(peek_count);
                if next_line > start_line && next_indent > start_indent {
                    return matches!(next_token, Dot);
                }
            }
        }

        false
    }

    fn peek_token(&mut self) -> Option<Token> {
        self.lexer.peek()
    }

    fn peek_token_n(&mut self, n: usize) -> Option<Token> {
        self.lexer.peek_n(n)
    }

    fn consume_token(&mut self) -> Option<Token> {
        self.lexer.next()
    }

    fn push_node(&mut self, node: Node) -> Result<AstIndex, ParserError> {
        self.push_node_with_span(node, self.lexer.span())
    }

    fn push_node_with_start_span(
        &mut self,
        node: Node,
        start_span: Span,
    ) -> Result<AstIndex, ParserError> {
        self.push_node_with_span(node, self.span_with_start(start_span))
    }

    fn push_node_with_span(&mut self, node: Node, span: Span) -> Result<AstIndex, ParserError> {
        self.ast.push(node, span)
    }

    fn span_with_start(&self, start_span: Span) -> Span {
        Span {
            start: start_span.start,
            end: self.lexer.span().end,
        }
    }

    // Peeks past whitespace, comments, and newlines until the next token is found
    //
    // Tokens on following lines will only be returned if the expression context allows linebreaks.
    //
    // If expected indentation is specified in the expression context, then the next token
    // needs to have matching indentation, otherwise None is returned.
    fn peek_next_token(&mut self, context: &ExpressionContext) -> Option<(Token, usize)> {
        use Token::*;

        let mut peek_count = 0;
        let start_line = self.lexer.line_number();
        let start_indent = self.lexer.current_indent();

        while let Some(peeked) = self.peek_token_n(peek_count) {
            match peeked {
                Whitespace | NewLine | NewLineIndented | CommentMulti | CommentSingle => {}
                token => {
                    if self.lexer.peek_line_number(peek_count) == start_line {
                        return Some((token, peek_count));
                    } else if context.allow_linebreaks {
                        let peeked_indent = self.lexer.peek_indent(peek_count);
                        if let Some(expected_indent) = context.expected_indentation {
                            if peeked_indent == expected_indent {
                                return Some((token, peek_count));
                            } else {
                                return None;
                            }
                        } else if peeked_indent > start_indent {
                            return Some((token, peek_count));
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
            }

            peek_count += 1;
        }

        None
    }

    // Consumes whitespace, comments, and newlines up until the next token
    //
    // If the expression context's indentation is None, and indentation is found, then the
    // context will be updated to expect the new indentation.
    //
    // It's expected that a peek has been performed to check that the current expression context
    // allows for the token to be consumed, see peek_next_token().
    fn consume_next_token(&mut self, context: &mut ExpressionContext) -> Option<Token> {
        use Token::*;

        let start_line = self.lexer.line_number();

        while let Some(token) = self.lexer.next() {
            match token {
                Whitespace | NewLine | NewLineIndented | CommentMulti | CommentSingle => {}
                token => {
                    if self.lexer.line_number() > start_line
                        && context.allow_linebreaks
                        && context.expected_indentation.is_none()
                    {
                        context.expected_indentation = Some(self.lexer.current_indent());
                    }

                    return Some(token);
                }
            }
        }

        None
    }

    // Consumes whitespace, comments, and newlines up until the next token
    //
    // If the expression context's indentation is None, and indentation is found, then the
    // context will be updated to expect the new indentation.
    //
    // It's expected that a peek has been performed to check that the current expression context
    // allows for the token to be consumed, see peek_next_token().
    fn consume_until_next_token(&mut self, context: &mut ExpressionContext) -> Option<Token> {
        use Token::*;

        let start_line = self.lexer.line_number();

        while let Some(peeked) = self.peek_token_n(0) {
            match peeked {
                Whitespace | NewLine | NewLineIndented | CommentMulti | CommentSingle => {}
                token => {
                    if self.lexer.peek_line_number(0) > start_line
                        && context.allow_linebreaks
                        && context.expected_indentation.is_none()
                    {
                        context.expected_indentation = Some(self.lexer.peek_indent(0));
                    }

                    return Some(token);
                }
            }

            self.lexer.next();
        }

        None
    }

    // Peeks past whitespace on the same line until the next token is found
    fn peek_next_token_on_same_line(&mut self) -> Option<Token> {
        use Token::*;

        let mut peek_count = 0;

        while let Some(peeked) = self.peek_token_n(peek_count) {
            match peeked {
                Whitespace | CommentMulti => {}
                token => return Some(token),
            }

            peek_count += 1;
        }

        None
    }

    // Consumes whitespace on the same line up until the next token
    fn consume_until_next_token_on_same_line(&mut self) {
        use Token::*;

        while let Some(peeked) = self.peek_token() {
            match peeked {
                Whitespace | CommentMulti => {}
                _ => return,
            }

            self.lexer.next();
        }
    }

    // Consumes whitespace on the same line and returns the next token
    fn consume_next_token_on_same_line(&mut self) -> Option<Token> {
        use Token::*;

        while let Some(peeked) = self.peek_token() {
            match peeked {
                Whitespace | CommentMulti => {}
                _ => return self.lexer.next(),
            }

            self.lexer.next();
        }

        None
    }
}

fn operator_precedence(op: Token) -> Option<(u8, u8)> {
    use Token::*;
    let priority = match op {
        Or => (1, 2),
        And => (3, 4),
        // Chained comparisons require right-associativity
        Equal | NotEqual => (8, 7),
        Greater | GreaterOrEqual | Less | LessOrEqual => (10, 9),
        Add | Subtract => (11, 12),
        Multiply | Divide | Modulo => (13, 14),
        _ => return None,
    };
    Some(priority)
}

fn token_is_whitespace(op: Token) -> bool {
    use Token::*;
    matches!(
        op,
        Whitespace | NewLine | NewLineIndented | CommentSingle | CommentMulti
    )
}
