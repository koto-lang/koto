#![cfg_attr(feature = "panic_on_parser_error", allow(unreachable_code))]

use {
    crate::{constant_pool::ConstantPoolBuilder, error::*, *},
    koto_lexer::{Lexer, Span, Token},
    std::{
        cmp::Ordering,
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

macro_rules! syntax_error {
    ($error:ident, $parser:expr) => {{
        let error = ParserError::new(SyntaxError::$error.into(), $parser.lexer.span());

        #[cfg(feature = "panic_on_parser_error")]
        panic!(error);

        #[cfg(not(feature = "panic_on_parser_error"))]
        Err(error)
    }};
}

fn f64_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < std::f64::EPSILON
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
    allow_function_start: bool,
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
}

impl ExpressionContext {
    fn line_start() -> Self {
        Self {
            allow_function_start: false,
            allow_linebreaks: true,
            allow_initial_indentation: false,
        }
    }

    fn permissive() -> Self {
        Self {
            allow_function_start: true,
            allow_linebreaks: true,
            allow_initial_indentation: true,
        }
    }

    fn restricted() -> Self {
        Self {
            allow_function_start: false,
            allow_linebreaks: false,
            allow_initial_indentation: false,
        }
    }

    fn inline() -> Self {
        Self {
            allow_function_start: true,
            allow_linebreaks: false,
            allow_initial_indentation: false,
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

        Ok((parser.ast, parser.constants.pool))
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

        let mut body = Vec::new();
        while self.consume_until_next_token().is_some() {
            if self.lexer.current_indent() == 0 {
                if let Some(expression) = self.parse_line()? {
                    body.push(expression);
                } else {
                    return syntax_error!(ExpectedExpressionInMainBlock, self);
                }
            } else {
                return syntax_error!(UnexpectedIndentation, self);
            }
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

    fn parse_function(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let current_indent = self.lexer.current_indent();

        if self.next_after_whitespace() != Some(Token::Function) {
            return internal_error!(FunctionParseFailure, self);
        }

        let span_start = self.lexer.span().start;

        // args
        let mut args = Vec::new();
        loop {
            self.consume_until_next_token();
            if let Some(constant_index) = self.parse_id(true) {
                args.push(constant_index);
            } else {
                break;
            }
        }

        if self.next_after_whitespace() != Some(Token::Function) {
            return syntax_error!(ExpectedFunctionArgsEnd, self);
        }

        // body
        let mut function_frame = Frame::default();
        function_frame.ids_assigned_in_scope.extend(args.clone());
        self.frame_stack.push(function_frame);

        let body = match self.peek_after_whitespace() {
            Some(Token::NewLineIndented) if self.lexer.next_indent() > current_indent => {
                if let Some(block) = self.parse_indented_map_or_block(current_indent)? {
                    block
                } else {
                    return internal_error!(FunctionParseFailure, self);
                }
            }
            _ => {
                self.consume_whitespace_on_same_line();
                if let Some(body) = self.parse_line()? {
                    body
                } else {
                    return syntax_error!(ExpectedFunctionBody, self);
                }
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
        let context = ExpressionContext::line_start();

        let result = if let Some(for_loop) = self.parse_for_loop(None)? {
            for_loop
        } else if let Some(loop_block) = self.parse_loop_block()? {
            loop_block
        } else if let Some(while_loop) = self.parse_while_loop(None)? {
            while_loop
        } else if let Some(until_loop) = self.parse_until_loop(None)? {
            until_loop
        } else if let Some(export_id) = self.parse_export_id()? {
            export_id
        } else if let Some(debug_expression) = self.parse_debug_expression()? {
            debug_expression
        } else if let Some(result) = self.parse_expressions(&context)? {
            result
        } else {
            return Ok(None);
        };

        self.frame_mut()?.finish_expressions();

        Ok(Some(result))
    }

    fn parse_expressions(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let current_indent = self.lexer.current_indent();

        let mut expected_indent = None;

        if context.allow_initial_indentation
            && self.peek_after_whitespace() == Some(Token::NewLineIndented)
        {
            self.consume_until_next_token();

            let indent = self.lexer.current_indent();
            if indent <= current_indent {
                return Ok(None);
            }

            expected_indent = Some(indent);

            if let Some(map_block) = self.parse_map_block(current_indent, expected_indent)? {
                return Ok(Some(map_block));
            }
        }

        let context = ExpressionContext {
            allow_function_start: true,
            ..*context
        };

        if let Some(first) = self.parse_expression(&context)? {
            let mut expressions = vec![first];
            while let Some(Token::Separator) = self.peek_after_whitespace() {
                self.next_after_whitespace();

                if self.peek_after_whitespace() == Some(Token::NewLineIndented) {
                    self.consume_until_next_token();

                    let next_indent = self.lexer.next_indent();

                    if let Some(expected_indent) = expected_indent {
                        match next_indent.cmp(&expected_indent) {
                            Ordering::Less => break,
                            Ordering::Equal => {}
                            Ordering::Greater => return syntax_error!(UnexpectedIndentation, self),
                        }
                    } else if next_indent <= current_indent {
                        break;
                    } else {
                        expected_indent = Some(next_indent);
                    }
                }

                if let Some(next_expression) =
                    self.parse_expression_with_lhs(Some(&expressions), &context)?
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
            if expressions.len() == 1 {
                Ok(Some(first))
            } else {
                Ok(Some(self.push_node(Node::Tuple(expressions))?))
            }
        } else {
            Ok(None)
        }
    }

    fn parse_expression_with_lhs(
        &mut self,
        lhs: Option<&[AstIndex]>,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression_start(lhs, 0, context)
    }

    fn parse_expression(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression_start(None, 0, context)
    }

    fn parse_expression_start(
        &mut self,
        lhs: Option<&[AstIndex]>,
        min_precedence: u8,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        let start_line = self.lexer.line_number();

        let expression_start = {
            // ID expressions are broken out to allow function calls in first position
            let expression = if let Some(expression) = self.parse_id_expression(context)? {
                Some(expression)
            } else {
                self.parse_term(context)?
            };

            match self.peek_token() {
                Some(Token::Range) | Some(Token::RangeInclusive) => {
                    return self.parse_range(expression)
                }
                _ => match expression {
                    Some(expression) => expression,
                    None => return Ok(None),
                },
            }
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
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        let last_lhs = match lhs {
            [last] => *last,
            [.., last] => *last,
            _ => return internal_error!(MissingContinuedExpressionLhs, self),
        };

        if let Some(next) = self.peek_after_whitespace() {
            match next {
                NewLine | NewLineIndented => {
                    if let Some((maybe_operator, _)) = self.peek_until_next_token() {
                        if operator_precedence(maybe_operator).is_some() {
                            self.consume_until_next_token();
                            return self.parse_expression_continued(lhs, min_precedence, context);
                        }
                    }
                }
                // TODO what's the correct context here?
                For if context.allow_function_start => return self.parse_for_loop(Some(lhs)),
                While if context.allow_function_start => return self.parse_while_loop(Some(lhs)),
                Until if context.allow_function_start => return self.parse_until_loop(Some(lhs)),
                Assign => return self.parse_assign_expression(lhs, AssignOp::Equal),
                AssignAdd => return self.parse_assign_expression(lhs, AssignOp::Add),
                AssignSubtract => return self.parse_assign_expression(lhs, AssignOp::Subtract),
                AssignMultiply => return self.parse_assign_expression(lhs, AssignOp::Multiply),
                AssignDivide => return self.parse_assign_expression(lhs, AssignOp::Divide),
                AssignModulo => return self.parse_assign_expression(lhs, AssignOp::Modulo),
                _ => {
                    if let Some((left_priority, right_priority)) = operator_precedence(next) {
                        if let Some(token_after_op) = self.peek_two_after_whitespace() {
                            if token_is_whitespace(token_after_op)
                                && left_priority >= min_precedence
                            {
                                let op = self.next_after_whitespace().unwrap();

                                let current_indent = self.lexer.current_indent();

                                let rhs = if let Some(map_block) =
                                    self.parse_map_block(current_indent, None)?
                                {
                                    map_block
                                } else if let Some(rhs_expression) =
                                    self.parse_expression_start(None, right_priority, context)?
                                {
                                    rhs_expression
                                } else {
                                    return syntax_error!(ExpectedRhsExpression, self);
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
        self.consume_token();

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
                Node::Lookup(_) => {}
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

        if let Some(rhs) = self.parse_expressions(&ExpressionContext::permissive())? {
            let node = if targets.len() == 1 {
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
            syntax_error!(ExpectedRhsExpression, self)
        }
    }

    fn parse_id(&mut self, allow_wildcards: bool) -> Option<ConstantIndex> {
        match self.peek_after_whitespace() {
            Some(Token::Id) => {
                self.next_after_whitespace();
                Some(self.constants.add_string(self.lexer.slice()) as u32)
            }
            Some(Token::Wildcard) if allow_wildcards => {
                self.next_after_whitespace();
                Some(self.constants.add_string(self.lexer.slice()) as u32)
            }
            _ => None,
        }
    }

    fn parse_id_or_string(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let result = match self.peek_after_whitespace() {
            Some(Token::Id) => {
                self.next_after_whitespace();
                Some(self.constants.add_string(self.lexer.slice()) as u32)
            }
            Some(Token::String) => {
                self.next_after_whitespace();
                let s = self.parse_string(self.lexer.slice())?;
                Some(self.constants.add_string(&s) as u32)
            }
            _ => None,
        };
        Ok(result)
    }

    fn parse_space_separated_call_args(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Vec<AstIndex>, ParserError> {
        let start_indent = self.lexer.current_indent();
        let mut current_line = self.lexer.line_number();
        let mut args_indent = None;
        let mut args = Vec::new();

        let arg_context = ExpressionContext {
            allow_function_start: false,
            ..*context
        };

        while let Some((_, peek_count)) = self.peek_until_next_token() {
            let peeked_line = self.lexer.peek_line_number(peek_count);
            if peeked_line > current_line {
                current_line = peeked_line;

                if context.allow_linebreaks {
                    if let Some(args_indent) = args_indent {
                        match self.lexer.peek_indent(peek_count).cmp(&args_indent) {
                            Ordering::Less => break,
                            Ordering::Equal => {}
                            Ordering::Greater => return syntax_error!(UnexpectedIndentation, self),
                        }
                    } else {
                        let next_indent = self.lexer.peek_indent(peek_count);
                        if next_indent > start_indent {
                            args_indent = Some(next_indent);
                        } else {
                            break;
                        }
                    }

                    self.consume_until_next_token();
                } else {
                    break;
                }
            } else if self.peek_token() == Some(Token::Whitespace) {
                self.consume_token();
            } else {
                break;
            }

            if let Some(expression) = self.parse_expression(&arg_context)? {
                args.push(expression);
            } else {
                break;
            }
        }

        Ok(args)
    }

    fn parse_id_expression(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        if let Some(constant_index) = self.parse_id(true) {
            self.frame_mut()?
                .increment_expression_access_for_id(constant_index);

            let id_index = self.push_node(Node::Id(constant_index))?;
            let result = match self.peek_token() {
                Some(Token::Whitespace) if context.allow_function_start => {
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
                Some(_) if context.allow_function_start && context.allow_linebreaks => {
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
        context: &ExpressionContext,
    ) -> Result<AstIndex, ParserError> {
        let mut lookup = Vec::new();

        let start_indent = self.lexer.current_indent();
        let mut lookup_indent = None;
        let mut node_context = *context;
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

                    let index_context = ExpressionContext {
                        allow_function_start: false, // TODO true?
                        allow_linebreaks: false,
                        allow_initial_indentation: false,
                    };

                    let index_expression =
                        if let Some(index_expression) = self.parse_expression(&index_context)? {
                            match self.peek_token() {
                                Some(Token::Range) => {
                                    self.consume_token();

                                    if let Some(end_expression) =
                                        self.parse_expression(&index_context)?
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
                                        self.parse_expression(&index_context)?
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
                            match self.peek_after_whitespace() {
                                Some(Token::Range) => {
                                    self.next_after_whitespace();

                                    if let Some(end_expression) =
                                        self.parse_expression(&index_context)?
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
                                    self.next_after_whitespace();

                                    if let Some(end_expression) =
                                        self.parse_expression(&index_context)?
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

                    if let Some(Token::ListEnd) = self.peek_after_whitespace() {
                        self.next_after_whitespace();
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
                Token::Whitespace if node_context.allow_function_start => {
                    let args = self.parse_space_separated_call_args(context)?;

                    if args.is_empty() {
                        break;
                    } else {
                        lookup.push((LookupNode::Call(args), node_start_span));

                        node_context = ExpressionContext {
                            allow_function_start: false,
                            ..node_context
                        };
                    }
                }
                _ if self.next_token_is_lookup_continuation() => {
                    self.consume_until_next_token();
                    let new_indent = self.lexer.current_indent();

                    if lookup_indent.is_none() {
                        if new_indent > start_indent {
                            lookup_indent = Some(new_indent);
                        } else {
                            break;
                        }
                    }

                    node_context = ExpressionContext {
                        allow_function_start: true,
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
        if self.next_after_whitespace() != Some(Token::ParenOpen) {
            return internal_error!(ArgumentsParseFailure, self);
        }

        let mut args = Vec::new();

        loop {
            self.consume_until_next_token();

            if let Some(expression) = self.parse_expression(&ExpressionContext::inline())? {
                args.push(expression);
            } else {
                break;
            }
        }

        self.consume_until_next_token();
        if self.consume_token() == Some(Token::ParenClose) {
            Ok(args)
        } else {
            syntax_error!(ExpectedArgsEnd, self)
        }
    }

    fn parse_range(&mut self, lhs: Option<AstIndex>) -> Result<Option<AstIndex>, ParserError> {
        use Node::{Range, RangeFrom, RangeFull, RangeTo};

        let inclusive = match self.peek_token() {
            Some(Token::Range) => false,
            Some(Token::RangeInclusive) => true,
            _ => return Ok(None),
        };

        self.consume_token();

        let rhs = self.parse_term(&ExpressionContext::restricted())?;

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

    fn parse_export_id(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() == Some(Token::Export) {
            self.next_after_whitespace();

            if let Some(constant_index) = self.parse_id(false) {
                let export_id = self.push_node(Node::Id(constant_index))?;

                match self.peek_after_whitespace() {
                    Some(Token::Assign) => {
                        self.next_after_whitespace();

                        let context = ExpressionContext::permissive();

                        if let Some(rhs) = self.parse_expressions(&context)? {
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
                            return syntax_error!(ExpectedRhsExpression, self);
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
        if self.peek_after_whitespace() != Some(Token::Debug) {
            return Ok(None);
        }

        self.next_after_whitespace();

        let start_position = self.lexer.span().start;

        self.consume_whitespace_on_same_line();

        let context = ExpressionContext::permissive();
        let expression_source_start = self.lexer.source_position();
        let expression = if let Some(expression) = self.parse_expressions(&context)? {
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

    fn parse_term(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>, ParserError> {
        use Node::*;

        let current_indent = self.lexer.current_indent();

        if let Some(token) = self.peek_after_whitespace() {
            let result = match token {
                Token::True => {
                    self.next_after_whitespace();
                    self.push_node(BoolTrue)?
                }
                Token::False => {
                    self.next_after_whitespace();
                    self.push_node(BoolFalse)?
                }
                Token::ParenOpen => return self.parse_nested_expressions(context),
                Token::Number => {
                    self.next_after_whitespace();
                    let number_node = match f64::from_str(self.lexer.slice()) {
                        Ok(n) => {
                            if f64_eq(n, 0.0) {
                                self.push_node(Number0)?
                            } else if f64_eq(n, 1.0) {
                                self.push_node(Number1)?
                            } else {
                                let constant_index = self.constants.add_f64(n) as u32;
                                self.push_node(Number(constant_index))?
                            }
                        }
                        Err(_) => {
                            return internal_error!(NumberParseFailure, self);
                        }
                    };
                    if self.next_token_is_lookup_start(context) {
                        self.parse_lookup(number_node, context)?
                    } else {
                        number_node
                    }
                }
                Token::String => {
                    self.next_after_whitespace();
                    let s = self.parse_string(self.lexer.slice())?;
                    let constant_index = self.constants.add_string(&s) as u32;
                    let string_node = self.push_node(Str(constant_index))?;
                    if self.next_token_is_lookup_start(context) {
                        self.parse_lookup(string_node, context)?
                    } else {
                        string_node
                    }
                }
                Token::Id => return self.parse_id_expression(context),
                Token::ListStart => return self.parse_list(context),
                Token::MapStart => return self.parse_map_inline(context),
                Token::Num2 => {
                    self.next_after_whitespace();
                    let start_span = self.lexer.span();

                    let args = if self.peek_token() == Some(Token::ParenOpen) {
                        self.parse_parenthesized_args()?
                    } else {
                        let mut args = Vec::new();
                        while let Some(arg) = self.parse_term(&ExpressionContext::restricted())? {
                            args.push(arg);
                        }
                        args
                    };

                    if args.is_empty() {
                        return syntax_error!(ExpectedExpression, self);
                    } else if args.len() > 2 {
                        return syntax_error!(TooManyNum2Terms, self);
                    }

                    self.push_node_with_start_span(Num2(args), start_span)?
                }
                Token::Num4 => {
                    self.next_after_whitespace();
                    let start_span = self.lexer.span();

                    let args = if self.peek_token() == Some(Token::ParenOpen) {
                        self.parse_parenthesized_args()?
                    } else {
                        let mut args = Vec::new();
                        while let Some(arg) = self.parse_term(&ExpressionContext::restricted())? {
                            args.push(arg);
                        }
                        args
                    };

                    if args.is_empty() {
                        return syntax_error!(ExpectedExpression, self);
                    } else if args.len() > 4 {
                        return syntax_error!(TooManyNum4Terms, self);
                    }

                    self.push_node_with_start_span(Num4(args), start_span)?
                }
                Token::If if context.allow_function_start => return self.parse_if_expression(),
                Token::Match => return self.parse_match_expression(),
                Token::Function => return self.parse_function(),
                Token::Copy => {
                    self.next_after_whitespace();
                    if let Some(expression) = self.parse_expression(&ExpressionContext {
                        allow_function_start: true,
                        ..*context
                    })? {
                        self.push_node(Node::CopyExpression(expression))?
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Subtract => {
                    if let Some(token_after_subtract) = self.peek_two_after_whitespace() {
                        if !token_is_whitespace(token_after_subtract) {
                            self.next_after_whitespace();
                            if let Some(term) = self.parse_term(&ExpressionContext::restricted())? {
                                self.push_node(Node::Negate(term))?
                            } else {
                                return syntax_error!(ExpectedExpression, self);
                            }
                        } else {
                            return Ok(None);
                        }
                    } else {
                        return Ok(None);
                    }
                }
                Token::Not => {
                    self.next_after_whitespace();
                    if let Some(expression) = self.parse_expression(&ExpressionContext {
                        allow_function_start: true,
                        ..*context
                    })? {
                        self.push_node(Node::Negate(expression))?
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Type => {
                    self.next_after_whitespace();
                    if let Some(expression) = self.parse_expression(&ExpressionContext {
                        allow_function_start: true,
                        ..*context
                    })? {
                        self.push_node(Node::Type(expression))?
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Yield => {
                    self.next_after_whitespace();
                    if let Some(expression) = self.parse_expressions(&ExpressionContext {
                        allow_function_start: true,
                        ..*context
                    })? {
                        let result = self.push_node(Node::Yield(expression))?;
                        self.frame_mut()?.contains_yield = true;
                        result
                    } else {
                        return syntax_error!(ExpectedExpression, self);
                    }
                }
                Token::Break => {
                    self.next_after_whitespace();
                    self.push_node(Node::Break)?
                }
                Token::Continue => {
                    self.next_after_whitespace();
                    self.push_node(Node::Continue)?
                }
                Token::Return => {
                    self.next_after_whitespace();
                    if let Some(expression) = self.parse_expressions(&ExpressionContext {
                        allow_function_start: true,
                        ..*context
                    })? {
                        self.push_node(Node::ReturnExpression(expression))?
                    } else {
                        self.push_node(Node::Return)?
                    }
                }
                Token::From | Token::Import => return self.parse_import_expression(),
                Token::Try if context.allow_function_start => return self.parse_try_expression(),
                Token::NewLineIndented => return self.parse_map_block(current_indent, None),
                Token::Error => return syntax_error!(LexerError, self),
                _ => return Ok(None),
            };

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn parse_list(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>, ParserError> {
        self.next_after_whitespace();
        let start_span = self.lexer.span();

        let lexer_reset_state = self.lexer.clone();
        let ast_reset_point = self.ast.reset_point();

        // A comprehension has to be parsed differently to a plain list of entries.
        // Any expression can appear at the start as the inline body of a loop, so first look for
        // any kind of expression, and then see if a loop follows.
        let list_comprehension =
            if let Some(expression) = self.parse_expression(&ExpressionContext::inline())? {
                match self.ast.node(expression).node {
                    Node::For(_) | Node::While { .. } | Node::Until { .. } => Some(expression),
                    _ => {
                        let loop_body = vec![expression];
                        if let Some(for_loop) = self.parse_for_loop(Some(&loop_body))? {
                            Some(for_loop)
                        } else if let Some(while_loop) = self.parse_while_loop(Some(&loop_body))? {
                            Some(while_loop)
                        } else if let Some(until_loop) = self.parse_until_loop(Some(&loop_body))? {
                            Some(until_loop)
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            };

        let entries = match list_comprehension {
            Some(comprehension) => vec![comprehension],
            None => {
                // No comprehension was found, so reset the lexer and AST to where things were
                // before trying to parse a comprehension, and then parse list entries.
                self.lexer = lexer_reset_state;
                self.ast.reset(ast_reset_point);

                let mut entries = Vec::new();

                let entry_context = ExpressionContext::inline();

                while self.consume_until_next_token() != Some(Token::ListEnd) {
                    if let Some(entry) = self.parse_expression(&entry_context)? {
                        entries.push(entry);
                    }

                    if self.peek_after_whitespace() == Some(Token::Separator) {
                        self.next_after_whitespace();
                    } else {
                        break;
                    }
                }

                entries
            }
        };

        self.consume_until_next_token();
        if self.consume_token() != Some(Token::ListEnd) {
            return syntax_error!(ExpectedListEnd, self);
        }

        let list_node = self.push_node_with_start_span(Node::List(entries), start_span)?;

        let result = if self.next_token_is_lookup_start(context) {
            self.parse_lookup(list_node, context)?
        } else {
            list_node
        };

        Ok(Some(result))
    }

    fn parse_indented_map_or_block(
        &mut self,
        current_indent: usize,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.consume_until_next_token();
        let expected_indent = self.lexer.next_indent();

        let result =
            if let Some(map_block) = self.parse_map_block(current_indent, Some(expected_indent))? {
                Some(map_block)
            } else if let Some(block) =
                self.parse_indented_block(current_indent, Some(expected_indent))?
            {
                Some(block)
            } else {
                None
            };

        Ok(result)
    }

    fn parse_map_block(
        &mut self,
        current_indent: usize,
        block_indent: Option<usize>,
    ) -> Result<Option<AstIndex>, ParserError> {
        let block_indent = match block_indent {
            Some(indent) => indent,
            None => {
                if self.peek_after_whitespace() != Some(Token::NewLineIndented) {
                    return Ok(None);
                }

                let block_indent = self.lexer.next_indent();

                if block_indent <= current_indent {
                    return Ok(None);
                }

                self.next_after_whitespace();
                block_indent
            }
        };

        // Look ahead to check there's at least one map entry
        if self.consume_until_next_token() != Some(Token::Id) {
            return Ok(None);
        }
        if self.peek_two_after_whitespace() != Some(Token::Colon) {
            return Ok(None);
        }

        let start_span = self.lexer.span();

        let mut entries = Vec::new();

        while let Some(key) = self.parse_id_or_string()? {
            if self.next_after_whitespace() == Some(Token::Colon) {
                if let Some(value) = self.parse_line()? {
                    entries.push((key, Some(value)));
                } else {
                    // If a value wasn't found on the same line as the key, scan ahead to the next
                    // token (skipping newlines) and try again
                    self.consume_until_next_token();
                    if let Some(value) = self.parse_line()? {
                        entries.push((key, Some(value)));
                    } else {
                        return syntax_error!(ExpectedMapValue, self);
                    }
                }
            } else {
                entries.push((key, None));
            }

            self.consume_until_next_token();

            let next_indent = self.lexer.next_indent();
            match next_indent.cmp(&block_indent) {
                Ordering::Less => break,
                Ordering::Equal => {}
                Ordering::Greater => return syntax_error!(UnexpectedIndentation, self),
            }
        }

        Ok(Some(self.push_node_with_start_span(
            Node::Map(entries),
            start_span,
        )?))
    }

    fn parse_map_inline(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        self.next_after_whitespace();
        let start_span = self.lexer.span();

        let mut entries = Vec::new();

        let value_context = ExpressionContext::permissive();

        loop {
            self.consume_until_next_token();

            if let Some(key) = self.parse_id_or_string()? {
                if self.peek_token() == Some(Token::Colon) {
                    self.consume_token();
                    self.consume_until_next_token();
                    if let Some(value) = self.parse_expression(&value_context)? {
                        entries.push((key, Some(value)));
                    } else {
                        return syntax_error!(ExpectedMapValue, self);
                    }
                } else {
                    entries.push((key, None));
                }

                if self.peek_after_whitespace() == Some(Token::Separator) {
                    self.next_after_whitespace();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if self.next_after_whitespace() != Some(Token::MapEnd) {
            return syntax_error!(ExpectedMapEnd, self);
        }

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
        inline_body: Option<&[AstIndex]>,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::For) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();

        self.next_after_whitespace();

        let mut args = Vec::new();
        while let Some(constant_index) = self.parse_id(true) {
            args.push(constant_index);
            self.frame_mut()?
                .ids_assigned_in_scope
                .insert(constant_index);

            match self.peek_after_whitespace() {
                Some(Token::Separator) => {
                    self.next_after_whitespace();
                }
                Some(Token::In) => {
                    self.next_after_whitespace();
                    break;
                }
                _ => return syntax_error!(ExpectedForInKeyword, self),
            }
        }
        if args.is_empty() {
            return syntax_error!(ExpectedForArgs, self);
        }

        let range_context = ExpressionContext::inline();
        let mut ranges = Vec::new();
        while let Some(range) = self.parse_expression(&range_context)? {
            ranges.push(range);

            if self.peek_after_whitespace() != Some(Token::Separator) {
                break;
            }

            self.next_after_whitespace();
        }
        if ranges.is_empty() {
            return syntax_error!(ExpectedForRanges, self);
        }

        let condition = if self.peek_after_whitespace() == Some(Token::If) {
            self.next_after_whitespace();
            let condition_context = ExpressionContext::inline();
            if let Some(condition) = self.parse_expression(&condition_context)? {
                Some(condition)
            } else {
                return syntax_error!(ExpectedForCondition, self);
            }
        } else {
            None
        };

        let body = if let Some(expressions) = inline_body {
            match expressions {
                [] => return internal_error!(ForParseFailure, self),
                [expression] => *expression,
                _ => self.push_node(Node::Tuple(expressions.to_vec()))?,
            }
        } else if let Some(body) = self.parse_indented_block(current_indent, None)? {
            body
        } else {
            return syntax_error!(ExpectedForBody, self);
        };

        let result = self.push_node(Node::For(AstFor {
            args,
            ranges,
            condition,
            body,
        }))?;

        Ok(Some(result))
    }

    fn parse_loop_block(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::Loop) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();
        self.next_after_whitespace();

        if let Some(body) = self.parse_indented_block(current_indent, None)? {
            let result = self.push_node(Node::Loop { body })?;
            Ok(Some(result))
        } else {
            return syntax_error!(ExpectedLoopBody, self);
        }
    }

    fn parse_while_loop(
        &mut self,
        inline_body: Option<&[AstIndex]>,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::While) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();
        self.next_after_whitespace();

        let condition =
            if let Some(condition) = self.parse_expression(&ExpressionContext::inline())? {
                condition
            } else {
                return syntax_error!(ExpectedWhileCondition, self);
            };

        let body = if let Some(expressions) = inline_body {
            match expressions {
                [] => return internal_error!(ForParseFailure, self),
                [expression] => *expression,
                _ => self.push_node(Node::Tuple(expressions.to_vec()))?,
            }
        } else if let Some(body) = self.parse_indented_block(current_indent, None)? {
            body
        } else {
            return syntax_error!(ExpectedWhileBody, self);
        };

        let result = self.push_node(Node::While { condition, body })?;
        Ok(Some(result))
    }

    fn parse_until_loop(
        &mut self,
        inline_body: Option<&[AstIndex]>,
    ) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::Until) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();
        self.next_after_whitespace();

        let condition =
            if let Some(condition) = self.parse_expression(&ExpressionContext::inline())? {
                condition
            } else {
                return syntax_error!(ExpectedUntilCondition, self);
            };

        let body = if let Some(expressions) = inline_body {
            match expressions {
                [] => return internal_error!(ForParseFailure, self),
                [expression] => *expression,
                _ => self.push_node(Node::Tuple(expressions.to_vec()))?,
            }
        } else if let Some(body) = self.parse_indented_block(current_indent, None)? {
            body
        } else {
            return syntax_error!(ExpectedUntilBody, self);
        };

        let result = self.push_node(Node::Until { condition, body })?;
        Ok(Some(result))
    }

    fn parse_if_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::If) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();

        let if_context = ExpressionContext::inline();
        self.next_after_whitespace();
        let condition = match self.parse_expression(&if_context)? {
            Some(condition) => condition,
            None => return syntax_error!(ExpectedIfCondition, self),
        };

        let result = if self.peek_after_whitespace() == Some(Token::Then) {
            self.next_after_whitespace();
            let then_node = match self.parse_expressions(&if_context)? {
                Some(then_node) => then_node,
                None => return syntax_error!(ExpectedThenExpression, self),
            };
            let else_node = if self.peek_after_whitespace() == Some(Token::Else) {
                self.next_after_whitespace();
                match self.parse_expressions(&if_context)? {
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
        } else if let Some(then_node) = self.parse_indented_map_or_block(current_indent)? {
            let mut else_if_blocks = Vec::new();

            while self.lexer.current_indent() == current_indent {
                if let Some(Token::ElseIf) = self.peek_after_whitespace() {
                    self.next_after_whitespace();
                    if let Some(else_if_condition) = self.parse_expression(&if_context)? {
                        if let Some(else_if_block) =
                            self.parse_indented_map_or_block(current_indent)?
                        {
                            else_if_blocks.push((else_if_condition, else_if_block));
                        } else {
                            return syntax_error!(ExpectedElseIfBlock, self);
                        }
                    } else {
                        return syntax_error!(ExpectedElseIfCondition, self);
                    }
                } else {
                    break;
                }
            }

            let else_node = if self.lexer.current_indent() == current_indent {
                if let Some(Token::Else) = self.peek_after_whitespace() {
                    self.next_after_whitespace();
                    if let Some(else_block) = self.parse_indented_map_or_block(current_indent)? {
                        Some(else_block)
                    } else {
                        return syntax_error!(ExpectedElseBlock, self);
                    }
                } else {
                    None
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
            return syntax_error!(ExpectedThenKeywordOrBlock, self);
        };

        Ok(Some(result))
    }

    fn parse_match_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_after_whitespace() != Some(Token::Match) {
            return Ok(None);
        }

        let current_indent = self.lexer.current_indent();
        let start_span = self.lexer.span();

        let match_context = ExpressionContext::inline();
        self.next_after_whitespace();
        let expression = match self.parse_expressions(&match_context)? {
            Some(expression) => expression,
            None => return syntax_error!(ExpectedMatchExpression, self),
        };

        self.consume_until_next_token();

        let match_indent = self.lexer.current_indent();
        if match_indent <= current_indent {
            return syntax_error!(ExpectedMatchArm, self);
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

                while let Some(Token::Separator) = self.peek_after_whitespace() {
                    self.next_after_whitespace();

                    match self.parse_match_pattern()? {
                        Some(pattern) => patterns.push(pattern),
                        None => return syntax_error!(ExpectedMatchPattern, self),
                    }
                }

                arm_patterns.push(match patterns.as_slice() {
                    [single_pattern] => *single_pattern,
                    _ => self.push_node(Node::Tuple(patterns))?,
                });

                if let Some(Token::Or) = self.peek_after_whitespace() {
                    self.next_after_whitespace();
                    expected_arm_count += 1;
                }
            }

            if arm_patterns.len() != expected_arm_count {
                return syntax_error!(ExpectedMatchPattern, self);
            }

            let condition = if self.peek_after_whitespace() == Some(Token::If) {
                self.next_after_whitespace();
                match self.parse_expression(&match_context)? {
                    Some(expression) => Some(expression),
                    None => return syntax_error!(ExpectedMatchCondition, self),
                }
            } else {
                None
            };

            let expression = if self.peek_after_whitespace() == Some(Token::Then) {
                self.next_after_whitespace();
                match self.parse_expressions(&match_context)? {
                    Some(expression) => expression,
                    None => return syntax_error!(ExpectedMatchArmExpressionAfterThen, self),
                }
            } else if let Some(indented_expression) =
                self.parse_indented_map_or_block(match_indent)?
            {
                indented_expression
            } else {
                return syntax_error!(ExpectedMatchArmExpression, self);
            };

            arms.push(MatchArm {
                patterns: arm_patterns,
                condition,
                expression,
            });

            self.consume_until_next_token();

            let next_indent = self.lexer.next_indent();
            match next_indent.cmp(&match_indent) {
                Ordering::Less => break,
                Ordering::Equal => {}
                Ordering::Greater => return syntax_error!(UnexpectedIndentation, self),
            }
        }

        Ok(Some(self.push_node_with_start_span(
            Node::Match { expression, arms },
            start_span,
        )?))
    }

    fn parse_match_pattern(&mut self) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        let pattern_context = ExpressionContext::restricted();
        let result = if let Some(token) = self.peek_after_whitespace() {
            match token {
                True | False | Number | String => return self.parse_term(&pattern_context),
                Id => match self.parse_id(false) {
                    Some(id) => {
                        self.frame_mut()?.ids_assigned_in_scope.insert(id);
                        Some(self.push_node(Node::Id(id))?)
                    }
                    None => return internal_error!(IdParseFailure, self),
                },
                ListStart => return self.parse_list(&pattern_context),
                Wildcard => {
                    self.next_after_whitespace();
                    Some(self.push_node(Node::Wildcard)?)
                }
                ParenOpen => {
                    if self.peek_two_after_whitespace() == Some(ParenClose) {
                        self.next_after_whitespace();
                        self.consume_token();
                        Some(self.push_node(Node::Empty)?)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        Ok(result)
    }

    fn parse_import_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let from_import = match self.peek_after_whitespace() {
            Some(Token::From) => true,
            Some(Token::Import) => false,
            _ => return internal_error!(UnexpectedToken, self),
        };

        self.next_after_whitespace();

        let start_span = self.lexer.span();

        let from = if from_import {
            let from = match self.consume_import_items()?.as_slice() {
                [from] => from.clone(),
                _ => return syntax_error!(ImportFromExpressionHasTooManyItems, self),
            };

            if self.peek_after_whitespace() != Some(Token::Import) {
                return syntax_error!(ExpectedImportKeywordAfterFrom, self);
            }
            self.next_after_whitespace();
            from
        } else {
            vec![]
        };

        let items = self.consume_import_items()?;
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

    fn parse_try_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        let current_indent = self.lexer.current_indent();
        self.next_after_whitespace();

        let start_span = self.lexer.span();

        let try_block = if let Some(try_block) = self.parse_indented_block(current_indent, None)? {
            try_block
        } else {
            return syntax_error!(ExpectedTryBody, self);
        };

        if self.next_after_whitespace() != Some(Token::Catch) {
            return syntax_error!(ExpectedCatchBlock, self);
        }

        let catch_arg = if let Some(catch_arg) = self.parse_id(true) {
            self.frame_mut()?.ids_assigned_in_scope.insert(catch_arg);
            catch_arg
        } else {
            return syntax_error!(ExpectedCatchArgument, self);
        };

        let catch_block =
            if let Some(catch_block) = self.parse_indented_block(current_indent, None)? {
                catch_block
            } else {
                return syntax_error!(ExpectedCatchBody, self);
            };

        let finally_block = if self.peek_after_whitespace() == Some(Token::Finally) {
            self.next_after_whitespace();
            if let Some(finally_block) = self.parse_indented_block(current_indent, None)? {
                Some(finally_block)
            } else {
                return syntax_error!(ExpectedFinallyBody, self);
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

        while let Some(item_root) = self.parse_id(false) {
            let mut item = vec![item_root];

            while self.peek_token() == Some(Token::Dot) {
                self.consume_token();

                match self.parse_id(false) {
                    Some(id) => item.push(id),
                    None => return syntax_error!(ExpectedImportModuleId, self),
                }
            }

            items.push(item);
        }

        if items.is_empty() {
            return syntax_error!(ExpectedIdInImportExpression, self);
        }

        Ok(items)
    }

    fn parse_indented_block(
        &mut self,
        current_indent: usize,
        block_indent: Option<usize>,
    ) -> Result<Option<AstIndex>, ParserError> {
        let block_indent = match block_indent {
            Some(indent) => indent,
            None => {
                if self.peek_after_whitespace() != Some(Token::NewLineIndented) {
                    return Ok(None);
                }

                if let Some((_, peek_count)) = self.peek_until_next_token() {
                    let block_indent = self.lexer.peek_indent(peek_count);

                    if block_indent <= current_indent {
                        return Ok(None);
                    }

                    block_indent
                } else {
                    return Ok(None);
                }
            }
        };

        if block_indent <= current_indent {
            return Ok(None);
        }

        let mut body = Vec::new();
        self.consume_until_next_token();

        let start_span = self.lexer.span();

        while let Some(expression) = self.parse_line()? {
            body.push(expression);

            self.consume_until_next_token();

            let next_indent = self.lexer.current_indent();
            match next_indent.cmp(&block_indent) {
                Ordering::Less => break,
                Ordering::Equal => {}
                Ordering::Greater => return syntax_error!(UnexpectedIndentation, self),
            }
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
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        if self.peek_after_whitespace() != Some(ParenOpen) {
            return Ok(None);
        }

        self.next_after_whitespace();

        let expression_context = ExpressionContext {
            allow_function_start: true,
            ..*context
        };
        let mut expressions = vec![];
        while let Some(expression) = self.parse_expression(&expression_context)? {
            expressions.push(expression);

            if self.peek_token() == Some(Token::Separator) {
                self.consume_token();
            } else {
                break;
            }
        }

        let result = match expressions.as_slice() {
            [] => self.push_node(Node::Empty)?,
            [single] => *single,
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

    fn next_token_is_lookup_start(&mut self, context: &ExpressionContext) -> bool {
        use Token::*;

        if matches!(
            self.peek_token(),
            Some(Dot) | Some(ListStart) | Some(ParenOpen)
        ) {
            return true;
        } else if context.allow_linebreaks {
            let start_line = self.lexer.line_number();
            let start_indent = self.lexer.current_indent();
            if let Some((next_token, peek_count)) = self.peek_until_next_token() {
                let next_line = self.lexer.peek_line_number(peek_count);
                let next_indent = self.lexer.peek_indent(peek_count);
                if next_line > start_line && next_indent > start_indent {
                    return matches!(next_token, Dot);
                }
            }
        }

        false
    }

    fn next_token_is_lookup_continuation(&mut self) -> bool {
        let start_line = self.lexer.line_number();
        if let Some((next_token, peek_count)) = self.peek_until_next_token() {
            let next_line = self.lexer.peek_line_number(peek_count);
            if next_line > start_line {
                return matches!(next_token, Token::Dot);
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
    fn peek_until_next_token(&mut self) -> Option<(Token, usize)> {
        use Token::*;

        let mut peek_count = 0;

        while let Some(peeked) = self.peek_token_n(peek_count) {
            match peeked {
                Whitespace | NewLine | NewLineIndented | NewLineSkipped | CommentMulti
                | CommentSingle => {}
                token => return Some((token, peek_count)),
            }

            peek_count += 1;
        }

        None
    }

    // Peeks past whitespace on the same line until the next token is found
    fn peek_after_whitespace(&mut self) -> Option<Token> {
        use Token::*;

        let mut peek_count = 0;

        while let Some(peeked) = self.peek_token_n(peek_count) {
            match peeked {
                Whitespace | NewLineSkipped => {}
                token => return Some(token),
            }

            peek_count += 1;
        }

        None
    }

    // Peeks the token after the one that would be found with peek_after_whitespace
    fn peek_two_after_whitespace(&mut self) -> Option<Token> {
        use Token::*;

        let mut peek_count = 0;
        let mut token_found = false;

        while let Some(peeked) = self.peek_token_n(peek_count) {
            if token_found {
                return Some(peeked);
            }

            match peeked {
                Whitespace | NewLineSkipped => {}
                _ => token_found = true,
            }

            peek_count += 1;
        }

        None
    }

    // Consumes whitespace, comments, and newlines up until the next token
    fn consume_until_next_token(&mut self) -> Option<Token> {
        use Token::*;

        while let Some(peeked) = self.peek_token() {
            match peeked {
                Whitespace | NewLine | NewLineIndented | NewLineSkipped | CommentMulti
                | CommentSingle => {}
                token => return Some(token),
            }

            self.lexer.next();
        }

        None
    }

    // Consumes whitespace on the same line up until the next token
    fn consume_whitespace_on_same_line(&mut self) {
        use Token::*;

        while let Some(peeked) = self.peek_token() {
            match peeked {
                Whitespace | NewLineSkipped => {}
                _ => return,
            }

            self.lexer.next();
        }
    }

    // Consumes whitespace on the same line and returns the next token
    fn next_after_whitespace(&mut self) -> Option<Token> {
        use Token::*;

        while let Some(peeked) = self.peek_token() {
            match peeked {
                Whitespace | NewLineSkipped => {}
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
    matches!(op, Whitespace | NewLine | NewLineIndented)
}
