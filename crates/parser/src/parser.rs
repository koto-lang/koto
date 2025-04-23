#![cfg_attr(feature = "panic_on_parser_error", allow(unreachable_code))]

use crate::{
    StringFormatOptions,
    ast::{Ast, AstIndex},
    constant_pool::{ConstantIndex, ConstantPoolBuilder},
    error::{Error, ErrorKind, ExpectedIndentation, InternalError, Result, SyntaxError},
    node::*,
};
use koto_lexer::{LexedToken, Lexer, Span, StringType, Token};
use std::{
    borrow::Cow,
    collections::HashSet,
    iter::Peekable,
    str::{Chars, FromStr},
};

// Contains info about the current frame, representing either the module's top level or a function
#[derive(Debug, Default)]
struct Frame {
    // If a frame contains yield then it represents a generator function
    contains_yield: bool,
    // IDs that have been assigned within the current frame
    ids_assigned_in_frame: HashSet<ConstantIndex>,
    // IDs and chain roots which were accessed when not locally assigned at the time of access
    accessed_non_locals: HashSet<ConstantIndex>,
    // While expressions are being parsed we keep track of lhs assignments and rhs accesses.
    // At the end of a multi-assignment expression (see `finalize_id_accesses`),
    // accessed IDs that weren't locally assigned at the time of access are then counted as
    // non-local accesses.
    pending_accesses: HashSet<ConstantIndex>,
    pending_assignments: HashSet<ConstantIndex>,
}

impl Frame {
    // The number of local values declared within the frame
    fn local_count(&self) -> usize {
        self.ids_assigned_in_frame.len()
    }

    // Non-locals accessed in a nested frame need to be declared as also accessed in this
    // frame. This ensures that captures from the outer frame will be available when
    // creating the nested inner frame.
    fn add_nested_accessed_non_locals(&mut self, nested_frame: &Frame) {
        for non_local in nested_frame.accessed_non_locals.iter() {
            if !self.pending_assignments.contains(non_local) {
                self.add_id_access(*non_local);
            }
        }
    }

    // Declare that an id has been accessed within the frame
    fn add_id_access(&mut self, id: ConstantIndex) {
        self.pending_accesses.insert(id);
    }

    // Declare that an id is being assigned to within the frame
    fn add_local_id_assignment(&mut self, id: ConstantIndex) {
        self.pending_assignments.insert(id);
        // While an assignment expression is being parsed, the LHS id is counted as an access
        // until the assignment operator is encountered.
        self.pending_accesses.remove(&id);
    }

    // At the end of an expression, determine which RHS accesses are non-local
    fn finalize_id_accesses(&mut self) {
        for id in self.pending_accesses.drain() {
            if !self.ids_assigned_in_frame.contains(&id) {
                self.accessed_non_locals.insert(id);
            }
        }

        self.ids_assigned_in_frame
            .extend(self.pending_assignments.drain());
    }
}

// The set of rules that can modify how an expression is parsed
#[derive(Clone, Copy, Debug)]
struct ExpressionContext {
    // e.g.
    //
    // match x
    //   foo.bar if x == 0 then...
    //
    // Without the flag, `if f == 0...` would be parsed as being an argument for a call to foo.bar.
    allow_space_separated_call: bool,
    // e.g. f = |x|
    //        x + x
    // This function can have an indented body.
    //
    // foo
    //   bar,
    //   baz
    // This function call can be broken over lines.
    //
    // while x < f y
    //   ...
    // Here, `f y` can't be broken over lines as the while expression expects an indented block.
    allow_linebreaks: bool,
    // When true, a map block is allowed in the current context.
    // e.g.
    //
    // x = foo: 42
    //        ^~~ A map block requires an indented block, so here the flag should be false
    //
    // return
    //   foo: 41
    //      ^~~ A colon following the foo identifier signifies the start of a map block.
    //          Consuming tokens through the indentation sets the flag to true,
    //          see consume_until_next_token()
    //
    // x = ||
    //   foo: 42
    //      ^~~ The first line in an indented block will have the flag set to true to allow the
    //          block to be parsed as a map, see parse_indented_block().
    allow_map_block: bool,
    // True when at the top-level inside braces, e.g. inside a tuple, list, or braced map
    inside_braces: bool,
    // The indentation rules for the current context
    expected_indentation: Indentation,
    // When true, map entries should be exported, with the keys assigned to local variables.
    // This is used in export expressions where a map is used to define the exported items.
    export_map_entries: bool,
}

// The indentation that should be expected on following lines for an expression to continue
#[derive(Clone, Copy, Debug)]
enum Indentation {
    // Indentation isn't required on following lines
    // (e.g. in a comma separated braced expression)
    Flexible,
    // Indentation should match the expected indentation
    // (e.g. in an indented block, each line should start with the same indentation)
    Equal(usize),
    // Indentation should be greater than the current indentation
    Greater,
    // Indentation should be greater than the specified indentation
    GreaterThan(usize),
    // Indentation should be greater than or equal to the specified indentation
    GreaterOrEqual(usize),
}

impl ExpressionContext {
    fn restricted() -> Self {
        Self {
            allow_space_separated_call: false,
            allow_linebreaks: false,
            allow_map_block: false,
            inside_braces: false,
            expected_indentation: Indentation::Greater,
            export_map_entries: false,
        }
    }

    fn permissive() -> Self {
        Self {
            allow_space_separated_call: true,
            allow_linebreaks: true,
            ..Self::restricted()
        }
    }

    fn inline() -> Self {
        Self {
            allow_space_separated_call: true,
            ..Self::restricted()
        }
    }

    // After a keyword like `yield` or `return`.
    // Like inline(), but inherits allow_linebreaks
    fn start_new_expression(&self) -> Self {
        Self {
            allow_linebreaks: self.allow_linebreaks,
            ..Self::inline()
        }
    }

    // Inside a braced expression
    //
    // - Space separated calls are only allowed with a single argument
    // - Inline function bodies can't return paren-free tuples
    fn inside_braces() -> Self {
        Self {
            expected_indentation: Indentation::Flexible,
            inside_braces: true,
            ..Self::permissive()
        }
    }

    // e.g.
    // [
    //   foo
    //     .bar()
    // # ^ here we're allowing an indented chain to be started
    // ]
    fn chain_start(&self) -> Self {
        use Indentation::*;

        let expected_indentation = match self.expected_indentation {
            Flexible | Equal(_) => Greater,
            other => other,
        };

        Self {
            allow_map_block: false,
            expected_indentation,
            export_map_entries: false,
            ..*self
        }
    }

    fn with_expected_indentation(&self, expected_indentation: Indentation) -> Self {
        Self {
            expected_indentation,
            ..*self
        }
    }

    fn with_exported_map_entries(&self) -> Self {
        Self {
            export_map_entries: true,
            ..*self
        }
    }
}

/// Koto's parser
pub struct Parser<'source> {
    source: &'source str,
    ast: Ast,
    constants: ConstantPoolBuilder,
    lexer: Lexer<'source>,
    current_token: LexedToken,
    current_line: u32,
    frame_stack: Vec<Frame>,
}

impl<'source> Parser<'source> {
    /// Takes in a source script, and produces an Ast
    pub fn parse(source: &'source str) -> Result<Ast> {
        let capacity_guess = source.len() / 4;
        let mut parser = Parser {
            source,
            ast: Ast::with_capacity(capacity_guess),
            constants: ConstantPoolBuilder::default(),
            lexer: Lexer::new(source),
            current_token: LexedToken::default(),
            current_line: 0,
            frame_stack: Vec::new(),
        };

        match parser.consume_main_block() {
            Ok(_) => {
                parser.ast.set_constants(parser.constants.build());
                Ok(parser.ast)
            }
            Err(error) => {
                #[cfg(feature = "error_ast")]
                {
                    parser.ast.set_constants(parser.constants.build());
                    let mut error = error;
                    error.ast = Some(Box::new(parser.ast));
                    Err(error)
                }
                #[cfg(not(feature = "error_ast"))]
                {
                    Err(error)
                }
            }
        }
    }

    // Parses the main 'top-level' block
    fn consume_main_block(&mut self) -> Result<AstIndex> {
        self.frame_stack.push(Frame::default());

        let start_span = self.current_span();

        let mut context = ExpressionContext::permissive();
        context.expected_indentation = Indentation::Equal(0);

        let mut body = AstVec::new();
        while self.peek_token_with_context(&context).is_some() {
            self.consume_until_token_with_context(&context);

            match self.parse_line(&ExpressionContext::permissive()) {
                Ok(Some(expression)) => {
                    body.push(expression);
                }
                Ok(None) => {
                    self.push_main_block_node_on_error(body, start_span);
                    return self.consume_token_and_error(SyntaxError::ExpectedExpression);
                }
                Err(error) => {
                    self.push_main_block_node_on_error(body, start_span);
                    return Err(error);
                }
            }

            match self.peek_next_token_on_same_line() {
                Some(Token::Semicolon) => {
                    self.consume_next_token_on_same_line();
                }
                Some(Token::NewLine) => {}
                None => break,
                _ => {
                    self.push_main_block_node_on_error(body, start_span);
                    return self.consume_token_and_error(SyntaxError::UnexpectedToken);
                }
            }
        }

        // Check that all tokens were consumed
        self.consume_until_token_with_context(&ExpressionContext::permissive());
        if self.peek_token().is_some() {
            self.push_main_block_node_on_error(body, start_span);
            return self.consume_token_and_error(SyntaxError::UnexpectedToken);
        }

        let result = self.push_main_block_node(body, start_span)?;

        self.frame_stack.pop();
        Ok(result)
    }

    fn push_main_block_node_on_error(&mut self, mut body: AstVec<AstIndex>, start_span: Span) {
        if cfg!(feature = "error_ast") {
            match (body.last(), self.ast.entry_point()) {
                (Some(last_successful), Some(last_errored)) if *last_successful != last_errored => {
                    body.push(last_errored)
                }
                (None, Some(last_errored)) => body.push(last_errored),
                _ => {}
            }
            self.push_main_block_node(body, start_span).ok();
        }
    }

    fn push_main_block_node(
        &mut self,
        body: AstVec<AstIndex>,
        start_span: Span,
    ) -> Result<AstIndex> {
        self.push_node_with_start_span(
            Node::MainBlock {
                body,
                local_count: self.frame()?.local_count(),
            },
            start_span,
        )
    }

    // Attempts to parse an indented block after the current position
    //
    // e.g.
    //   my_function = |x, y| # <- Here at entry
    //     x = y + 1          # | < indented block
    //     foo x              # | < indented block
    fn parse_indented_block(&mut self) -> Result<Option<AstIndex>> {
        let block_context = ExpressionContext::permissive();

        let start_indent = self.current_indent();
        match self.peek_token_with_context(&block_context) {
            Some(peeked) if peeked.info.indent > start_indent => {}
            _ => return Ok(None), // No indented block found
        }

        let block_context = self
            .consume_until_token_with_context(&block_context)
            .unwrap(); // Safe to unwrap here given that we've just peeked
        let start_span = self.current_span();

        let mut body = AstVec::new();
        loop {
            let line_context = ExpressionContext {
                allow_map_block: body.is_empty(),
                ..ExpressionContext::permissive()
            };

            match self.parse_line(&line_context) {
                Ok(Some(expression)) => {
                    body.push(expression);
                }
                Ok(None) => {
                    // At this point we've peeked to check that the line is either the start of the
                    // block, or a continuation with the same indentation as the block.
                    self.push_block_node_on_error(body, start_span);
                    return self.consume_token_and_error(SyntaxError::UnexpectedToken);
                }
                Err(error) => {
                    self.push_block_node_on_error(body, start_span);
                    return Err(error);
                }
            }

            match self.peek_next_token_on_same_line() {
                None => break,
                Some(Token::NewLine) => {}
                Some(Token::Semicolon) => {
                    self.consume_next_token_on_same_line();
                }
                _ => {
                    self.push_block_node_on_error(body, start_span);
                    return self.consume_token_and_error(SyntaxError::UnexpectedToken);
                }
            }

            // Peek ahead to see if the indented block continues after this line
            if self.peek_token_with_context(&block_context).is_none() {
                break;
            }

            self.consume_until_token_with_context(&block_context);
        }

        self.push_block_node(body, start_span)
    }

    fn push_block_node_on_error(&mut self, mut body: AstVec<AstIndex>, start_span: Span) {
        if cfg!(feature = "error_ast") {
            match (body.last(), self.ast.entry_point()) {
                (Some(last_successful), Some(last_errored)) if *last_successful != last_errored => {
                    body.push(last_errored)
                }
                (None, Some(last_errored)) => body.push(last_errored),
                _ => {}
            }
            self.push_block_node(body, start_span).ok();
        }
    }

    fn push_block_node(
        &mut self,
        body: AstVec<AstIndex>,
        start_span: Span,
    ) -> Result<Option<AstIndex>> {
        // If the block is a single expression then it doesn't need to be wrapped in a Block node
        if body.len() == 1 {
            Ok(Some(*body.first().unwrap()))
        } else {
            self.push_node_with_start_span(Node::Block(body), start_span)
                .map(Some)
        }
    }

    // Parses expressions from the start of a line
    fn parse_line(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>> {
        self.parse_expressions(context, TempResult::No)
    }

    // Parse a comma separated series of expressions
    //
    // If only a single expression is encountered then that expression's node is the result.
    //
    // Otherwise, for multiple expressions, the result of the expression can be temporary
    // (i.e. not assigned to an identifier) in which case a TempTuple is generated,
    // otherwise the result will be a Tuple.
    fn parse_expressions(
        &mut self,
        context: &ExpressionContext,
        temp_result: TempResult,
    ) -> Result<Option<AstIndex>> {
        let mut expression_context = ExpressionContext {
            allow_space_separated_call: true,
            ..*context
        };

        let start_line = self.current_line;

        let Some(first) = self.parse_expression(&expression_context)? else {
            return Ok(None);
        };

        let mut expressions = astvec![first];
        let mut encountered_linebreak = false;
        let mut last_token_was_a_comma = false;

        // If comma-separated expressions are allowed then parse them now
        //
        // When inside braces we don't
        if !context.inside_braces {
            while let Some(Token::Comma) = self.peek_next_token_on_same_line() {
                self.consume_next_token_on_same_line();

                if last_token_was_a_comma {
                    expressions.push(self.push_node(Node::Null)?);
                }

                last_token_was_a_comma = true;

                if !encountered_linebreak && self.current_line > start_line {
                    // e.g.
                    //   x, y =
                    //     1, # <- We're here,
                    //        #    following values should have at least this level of indentation.
                    //     0,
                    expression_context = expression_context.with_expected_indentation(
                        Indentation::GreaterOrEqual(self.current_indent()),
                    );
                    encountered_linebreak = true;
                }

                if let Some(next_expression) =
                    self.parse_expression_start(&expressions, 0, &expression_context)?
                {
                    last_token_was_a_comma = false;

                    match self.ast.node(next_expression).node {
                        Node::Assign { .. } | Node::MultiAssign { .. } => {
                            // Assignments will have consumed all of the comma-separated expressions
                            // encountered so far as the LHS of the assignment, so we can exit here.
                            return Ok(Some(next_expression));
                        }
                        _ => {}
                    }

                    let next_expression = match self.peek_next_token_on_same_line() {
                        Some(Token::Range | Token::RangeInclusive) => {
                            self.consume_range(Some(next_expression), context)?
                        }
                        _ => next_expression,
                    };

                    expressions.push(next_expression);
                }
            }
        }

        self.frame_mut()?.finalize_id_accesses();

        if expressions.len() == 1 && !last_token_was_a_comma {
            Ok(Some(first))
        } else {
            let result = match temp_result {
                TempResult::No => Node::Tuple(expressions),
                TempResult::Yes => Node::TempTuple(expressions),
            };
            Ok(Some(self.push_node(result)?))
        }
    }

    // Parses a single expression
    //
    // Unlike parse_expressions() (which will consume a comma-separated series of expressions),
    // parse_expression() will stop when a comma is encountered.
    fn parse_expression(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>> {
        self.parse_expression_with_min_precedence(0, context)
    }

    // Parses a single expression with a specified minimum operator precedence
    fn parse_expression_with_min_precedence(
        &mut self,
        min_precedence: u8,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>> {
        let result = self.parse_expression_start(&[], min_precedence, context)?;

        match self.peek_next_token_on_same_line() {
            Some(Token::Range | Token::RangeInclusive) => {
                self.consume_range(result, context).map(Some)
            }
            _ => Ok(result),
        }
    }

    // Parses a term, and then checks to see if the expression is continued
    //
    // When parsing comma-separated expressions, the previous expressions are passed in so that
    // if an assignment operator is encountered then the overall expression is treated as a
    // multi-assignment.
    fn parse_expression_start(
        &mut self,
        previous_expressions: &[AstIndex],
        min_precedence: u8,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>> {
        let entry_line = self.current_line;

        // Look ahead to get the indent of the first token in the expression.
        // We need to look ahead here because the term may contain its own indentation,
        // so it may end with different indentation.
        let Some(start_info) = self.peek_token_with_context(context) else {
            return Ok(None);
        };

        let Some(expression_start) = self.parse_term(context)? else {
            return Ok(None);
        };
        let start_span = *self.ast.span(self.ast.node(expression_start).span);

        let continuation_context = if self.current_line > entry_line {
            match context.expected_indentation {
                Indentation::Equal(indent)
                | Indentation::GreaterThan(indent)
                | Indentation::GreaterOrEqual(indent) => {
                    // If the context has a fixed indentation requirement, then allow the
                    // indentation for the continued expression to grow or stay the same
                    context.with_expected_indentation(Indentation::GreaterOrEqual(indent))
                }
                Indentation::Greater | Indentation::Flexible => {
                    // Indentation within an arithmetic expression shouldn't be able to continue
                    // with decreased indentation
                    context.with_expected_indentation(Indentation::GreaterOrEqual(
                        start_info.info.indent,
                    ))
                }
            }
        } else {
            *context
        };

        self.parse_expression_continued(
            expression_start,
            start_span,
            previous_expressions,
            min_precedence,
            &continuation_context,
        )
    }

    // Parses the continuation of an expression_context
    //
    // Checks for an operator, and then parses the following expressions as the RHS of a binary
    // operation.
    fn parse_expression_continued(
        &mut self,
        expression_start: AstIndex,
        start_span: Span,
        previous_expressions: &[AstIndex],
        min_precedence: u8,
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>> {
        let start_line = self.current_line;
        let start_indent = self.current_indent();

        if let Some(assignment_expression) =
            self.parse_assign_expression(expression_start, previous_expressions, context)?
        {
            return Ok(Some(assignment_expression));
        } else if let Some(next) = self.peek_token_with_context(context) {
            if let Some((left_priority, right_priority)) = operator_precedence(next.token) {
                if left_priority >= min_precedence {
                    let (op, _) = self.consume_token_with_context(context).unwrap();

                    // Move on to the token after the operator
                    if self.peek_token_with_context(context).is_none() {
                        return self.error(ExpectedIndentation::RhsExpression);
                    }
                    self.consume_until_token_with_context(context).unwrap();

                    let rhs_context = if self.current_line > start_line {
                        match context.expected_indentation {
                            Indentation::Equal(indent)
                            | Indentation::GreaterThan(indent)
                            | Indentation::GreaterOrEqual(indent) => {
                                // If the context has a fixed indentation requirement, then allow
                                // the indentation for the continued expression to grow or stay the
                                // same.
                                context
                                    .with_expected_indentation(Indentation::GreaterOrEqual(indent))
                            }
                            Indentation::Greater | Indentation::Flexible => {
                                // Indentation within an arithmetic expression shouldn't be able to
                                // continue with decreased indentation.
                                context.with_expected_indentation(Indentation::GreaterOrEqual(
                                    start_indent,
                                ))
                            }
                        }
                    } else {
                        *context
                    };
                    let Some(rhs) =
                        self.parse_expression_start(&[], right_priority, &rhs_context)?
                    else {
                        return self.error(ExpectedIndentation::RhsExpression);
                    };

                    use Token::*;
                    let ast_op = match op {
                        Add => AstBinaryOp::Add,
                        Subtract => AstBinaryOp::Subtract,
                        Multiply => AstBinaryOp::Multiply,
                        Divide => AstBinaryOp::Divide,
                        Remainder => AstBinaryOp::Remainder,
                        Power => AstBinaryOp::Power,

                        AddAssign => AstBinaryOp::AddAssign,
                        SubtractAssign => AstBinaryOp::SubtractAssign,
                        MultiplyAssign => AstBinaryOp::MultiplyAssign,
                        DivideAssign => AstBinaryOp::DivideAssign,
                        RemainderAssign => AstBinaryOp::RemainderAssign,
                        PowerAssign => AstBinaryOp::PowerAssign,

                        Equal => AstBinaryOp::Equal,
                        NotEqual => AstBinaryOp::NotEqual,

                        Greater => AstBinaryOp::Greater,
                        GreaterOrEqual => AstBinaryOp::GreaterOrEqual,
                        Less => AstBinaryOp::Less,
                        LessOrEqual => AstBinaryOp::LessOrEqual,

                        And => AstBinaryOp::And,
                        Or => AstBinaryOp::Or,

                        Arrow => AstBinaryOp::Pipe,

                        _ => unreachable!(), // The list of tokens here matches the operators in
                                             // operator_precedence()
                    };

                    let op_node = self.push_node_with_start_span(
                        Node::BinaryOp {
                            op: ast_op,
                            lhs: expression_start,
                            rhs,
                        },
                        start_span,
                    )?;

                    return self.parse_expression_continued(
                        op_node,
                        start_span,
                        &[],
                        min_precedence,
                        &rhs_context,
                    );
                }
            }
        }

        Ok(Some(expression_start))
    }

    // Parses an assignment expression
    //
    // In a multi-assignment expression the LHS can be a series of targets. The last target in the
    // series will be passed in as `lhs`, with the previous targets passed in as `previous_lhs`.
    //
    // If the assignment is an export then operators other than `=` will be rejected.
    fn parse_assign_expression(
        &mut self,
        lhs: AstIndex,
        previous_lhs: &[AstIndex],
        context: &ExpressionContext,
    ) -> Result<Option<AstIndex>> {
        match self
            .peek_token_with_context(context)
            .map(|token| token.token)
        {
            Some(Token::Assign) => {}
            _ => return Ok(None),
        }

        let mut targets = AstVec::with_capacity(previous_lhs.len() + 1);

        for lhs_expression in previous_lhs.iter().chain(std::iter::once(&lhs)) {
            // Note which identifiers are being assigned to
            match self.ast.node(*lhs_expression).node.clone() {
                Node::Id(id_index, ..) => {
                    self.frame_mut()?.add_local_id_assignment(id_index);
                }
                Node::Meta { .. } | Node::Chain(_) | Node::Wildcard(..) => {}
                _ => return self.error(SyntaxError::ExpectedAssignmentTarget),
            }

            targets.push(*lhs_expression);
        }

        if targets.is_empty() {
            return self.error(InternalError::MissingAssignmentTarget);
        }

        // Consume the `=` token
        self.consume_token_with_context(context);
        let assign_span = self.current_span();

        let single_target = targets.len() == 1;

        let temp_result = if single_target {
            TempResult::No
        } else {
            TempResult::Yes
        };

        if let Some(rhs) = self.parse_expressions(context, temp_result)? {
            let node = if single_target {
                Node::Assign {
                    target: *targets.first().unwrap(),
                    expression: rhs,
                }
            } else {
                Node::MultiAssign {
                    targets,
                    expression: rhs,
                }
            };
            Ok(Some(self.push_node_with_span(node, assign_span)?))
        } else {
            self.error(ExpectedIndentation::AssignmentExpression)
        }
    }

    // Peeks the next token and dispatches to the relevant parsing functions
    fn parse_term(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>> {
        use Node::*;

        let start_span = self.current_span();
        let start_indent = self.current_indent();

        let Some(peeked) = self.peek_token_with_context(context) else {
            return Ok(None);
        };

        let result = match peeked.token {
            Token::Null => {
                self.consume_token_with_context(context);
                self.push_node(Null)
            }
            Token::True => {
                self.consume_token_with_context(context);
                self.push_node(BoolTrue)
            }
            Token::False => {
                self.consume_token_with_context(context);
                self.push_node(BoolFalse)
            }
            Token::RoundOpen => self.consume_tuple(context),
            Token::Number => self.consume_number(false, context),
            Token::StringStart { .. } => {
                let string = self.parse_string(context)?.unwrap();
                let string_node = self.push_node_with_span(Str(string.string), string.span)?;

                if string.context.allow_map_block
                    && self.peek_next_token_on_same_line() == Some(Token::Colon)
                {
                    self.consume_map_block(string_node, start_span, &string.context)
                } else {
                    self.check_for_chain_after_node(string_node, &string.context)
                }
            }
            Token::Id => self.consume_id_expression(context),
            Token::Self_ => self.consume_self_expression(context),
            Token::At => {
                let map_block_allowed =
                    context.allow_map_block || peeked.info.indent > start_indent;

                let meta_context = self.consume_until_token_with_context(context).unwrap();
                // Safe to unwrap here, parse_meta_key would error on invalid key
                let meta_key = self.parse_meta_key()?.unwrap();

                if map_block_allowed
                    && matches!(
                        self.peek_token_with_context(context),
                        Some(PeekInfo {
                            token: Token::Colon,
                            ..
                        })
                    )
                {
                    self.consume_map_block(meta_key, start_span, &meta_context)
                } else {
                    match self.parse_assign_expression(meta_key, &[], &meta_context)? {
                        Some(result) => self.push_node(Node::Export(result)),
                        None => self
                            .consume_token_and_error(SyntaxError::ExpectedAssignmentAfterMetaKey),
                    }
                }
            }
            Token::Wildcard => {
                let maybe_id = self.consume_wildcard(context)?;
                self.push_node(Node::Wildcard(maybe_id, None))
            }
            Token::SquareOpen => self.consume_list(context),
            Token::CurlyOpen => self.consume_map_with_braces(context),
            Token::If => self.consume_if_expression(context),
            Token::Match => self.consume_match_expression(context),
            Token::Switch => self.consume_switch_expression(context),
            Token::Function => self.consume_function(context),
            Token::Subtract => match self.peek_token_n(peeked.peek_count + 1) {
                Some(token) if token.is_whitespace_including_newline() => return Ok(None),
                Some(Token::Number) => {
                    self.consume_token_with_context(context); // Token::Subtract
                    self.consume_number(true, context)
                }
                Some(_) => {
                    self.consume_token_with_context(context); // Token::Subtract
                    if let Some(term) = self.parse_term(&ExpressionContext::restricted())? {
                        self.push_node(Node::UnaryOp {
                            op: AstUnaryOp::Negate,
                            value: term,
                        })
                    } else {
                        self.consume_token_and_error(SyntaxError::ExpectedExpression)
                    }
                }
                None => return Ok(None),
            },
            Token::Not => {
                self.consume_token_with_context(context);
                if let Some(expression) = self.parse_expression(&ExpressionContext {
                    allow_space_separated_call: true,
                    expected_indentation: Indentation::Greater,
                    ..*context
                })? {
                    self.push_node(Node::UnaryOp {
                        op: AstUnaryOp::Not,
                        value: expression,
                    })
                } else {
                    self.consume_token_and_error(SyntaxError::ExpectedExpression)
                }
            }
            Token::Yield => {
                self.consume_token_with_context(context);
                let start_span = self.current_span();
                if let Some(expression) =
                    self.parse_expressions(&context.start_new_expression(), TempResult::No)?
                {
                    self.frame_mut()?.contains_yield = true;
                    self.push_node_with_start_span(Node::Yield(expression), start_span)
                } else {
                    self.consume_token_and_error(SyntaxError::ExpectedExpression)
                }
            }
            Token::Loop => self.consume_loop_block(context),
            Token::For => self.consume_for_loop(context),
            Token::While => self.consume_while_loop(context),
            Token::Until => self.consume_until_loop(context),
            Token::Break => {
                self.consume_token_with_context(context);
                let break_value =
                    self.parse_expressions(&context.start_new_expression(), TempResult::No)?;
                self.push_node(Node::Break(break_value))
            }
            Token::Continue => {
                self.consume_token_with_context(context);
                self.push_node(Node::Continue)
            }
            Token::Return => {
                self.consume_token_with_context(context);
                let start_span = self.current_span();
                let return_value =
                    self.parse_expressions(&context.start_new_expression(), TempResult::No)?;
                self.push_node_with_start_span(Node::Return(return_value), start_span)
            }
            Token::Throw => self.consume_throw_expression(),
            Token::Debug => self.consume_debug_expression(),
            Token::From | Token::Import => self.consume_import(context),
            Token::Export => self.consume_export(context),
            Token::Try => self.consume_try_expression(context),
            Token::Let => self.consume_let_expression(context),
            // Reserved keywords
            Token::Await => self.consume_token_and_error(SyntaxError::ReservedKeyword),
            Token::Const => self.consume_token_and_error(SyntaxError::ReservedKeyword),
            // An error occurred in the lexer
            Token::Error => self.consume_token_and_error(SyntaxError::UnexpectedToken),
            _ => return Ok(None),
        };

        result.map(Some)
    }

    // Parses a function
    //
    // e.g.
    //   f = |x, y| x + y
    //   #   ^ You are here
    fn consume_function(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::Function

        let start_span = self.current_span();

        // Parse the function's args
        let mut arg_nodes = AstVec::new();
        let mut arg_ids = AstVec::new();
        let mut is_variadic = false;
        let mut default_value_expected = false;
        let args_context = ExpressionContext::inside_braces();
        while self.peek_token_with_context(&args_context).is_some() {
            self.consume_until_token_with_context(&args_context);

            let maybe_id_or_wildcard = self.parse_id_or_wildcard(&args_context)?;
            let arg_span = self.current_span();

            let arg_node = match maybe_id_or_wildcard {
                Some(IdOrWildcard::Id(constant_index)) => {
                    arg_ids.push(constant_index);
                    let type_hint = self.parse_type_hint(&args_context)?;
                    let id_node =
                        self.push_node_with_span(Node::Id(constant_index, type_hint), arg_span)?;

                    if self.peek_token() == Some(Token::Ellipsis) {
                        if type_hint.is_some() {
                            return self.consume_token_and_error(SyntaxError::UnexpectedToken);
                        }
                        self.consume_token();
                        is_variadic = true;
                        arg_nodes.push(id_node);
                        // The variadic argument must be last
                        break;
                    }

                    id_node
                }
                Some(IdOrWildcard::Wildcard(maybe_id)) => {
                    let type_hint = self.parse_type_hint(&args_context)?;
                    self.push_node_with_span(Node::Wildcard(maybe_id, type_hint), arg_span)?
                }
                None => match self.peek_token() {
                    Some(Token::Self_) => {
                        self.consume_token();
                        return self.error(SyntaxError::SelfArg);
                    }
                    Some(Token::RoundOpen) => {
                        self.consume_token();
                        let nested_span_start = self.current_span();

                        let tuple_args = self.parse_nested_function_args(&mut arg_ids)?;
                        self.expect_and_consume_token(
                            Token::RoundClose,
                            SyntaxError::ExpectedCloseParen.into(),
                            &args_context,
                        )?;

                        self.push_node_with_start_span(Node::Tuple(tuple_args), nested_span_start)?
                    }
                    _ => break,
                },
            };

            // Default value?
            if matches!(
                self.peek_token_with_context(&args_context),
                Some(PeekInfo {
                    token: Token::Assign,
                    ..
                })
            ) {
                self.consume_token_with_context(&args_context); // =

                let Some(default_value) = self.parse_expression(&args_context)? else {
                    return self.error(ExpectedIndentation::AssignmentExpression);
                };

                arg_nodes.push(self.push_node_with_start_span(
                    Node::Assign {
                        target: arg_node,
                        expression: default_value,
                    },
                    arg_span,
                )?);

                default_value_expected = true;
            } else if default_value_expected {
                return self.error(SyntaxError::ExpectedDefaultValue);
            } else {
                arg_nodes.push(arg_node);
            }

            if matches!(
                self.peek_token_with_context(&args_context),
                Some(PeekInfo {
                    token: Token::Comma,
                    ..
                })
            ) {
                self.consume_token_with_context(&args_context);
            } else {
                break;
            }
        }

        // Check for function args end
        self.expect_and_consume_token(
            Token::Function,
            SyntaxError::ExpectedFunctionArgsEnd.into(),
            &args_context,
        )?;

        // Check for output type hint
        let output_type = if self.peek_next_token_on_same_line() == Some(Token::Arrow) {
            self.consume_token_with_context(&args_context); // ->
            self.consume_type(&args_context).map(Some)?
        } else {
            None
        };

        // Function body
        let mut function_frame = Frame::default();
        function_frame.ids_assigned_in_frame.extend(arg_ids.iter());
        self.frame_stack.push(function_frame);

        let body = match self.parse_indented_block() {
            Ok(Some(block)) => block,
            Ok(None) => {
                // No indented block, so an inline body is expected
                self.consume_until_next_token_on_same_line();
                // If the function is being defined inside braces then the inline body should reflect
                // that, in particular a paren-free comma belongs to the containing braces.
                // E.g., `[|x| x, |y| y]` should be parsed as:
                //   `[(|x| x), (|y| y)]`,
                // and not
                //   `[|x| (x, |y| y)]`.
                let body_context = if context.inside_braces {
                    ExpressionContext::inside_braces()
                } else {
                    ExpressionContext::permissive()
                };
                if let Some(body) = self.parse_line(&body_context)? {
                    body
                } else {
                    return self.consume_token_and_error(ExpectedIndentation::FunctionBody);
                }
            }
            Err(error) => {
                if cfg!(feature = "error_ast") {
                    if let Some(block) = self.ast.entry_point() {
                        self.push_function_node(
                            start_span,
                            arg_nodes,
                            is_variadic,
                            output_type,
                            block,
                        )
                        .ok();
                    }
                }
                return Err(error);
            }
        };

        self.push_function_node(start_span, arg_nodes, is_variadic, output_type, body)
    }

    fn push_function_node(
        &mut self,
        start_span: Span,
        arg_nodes: AstVec<AstIndex>,
        is_variadic: bool,
        output_type: Option<AstIndex>,
        body: AstIndex,
    ) -> std::result::Result<AstIndex, Error> {
        let function_frame = self
            .frame_stack
            .pop()
            .ok_or_else(|| self.make_error(InternalError::MissingFrame))?;

        self.frame_mut()?
            .add_nested_accessed_non_locals(&function_frame);

        let local_count = function_frame.local_count();

        self.push_node_with_start_span(
            Node::Function(Function {
                args: arg_nodes,
                local_count,
                accessed_non_locals: AstVec::from_iter(function_frame.accessed_non_locals),
                body,
                is_variadic,
                is_generator: function_frame.contains_yield,
                output_type,
            }),
            start_span,
        )
    }

    // Parses a type hint following an identifier
    // e.g.:
    // let x: String = 'hello'
    //      ^ You are here
    fn parse_type_hint(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>> {
        let Some(PeekInfo {
            token: Token::Colon,
            ..
        }) = self.peek_token_with_context(context)
        else {
            return Ok(None);
        };

        self.consume_token_with_context(context); // :
        self.consume_type(context).map(Some)
    }

    fn consume_type(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        let Some((type_index, _)) = self.parse_id(context)? else {
            return self.consume_token_and_error(SyntaxError::ExpectedType);
        };

        // Check for the start of a nested type
        if self.peek_next_token_on_same_line() == Some(Token::Less) {
            return self.consume_token_and_error(SyntaxError::NestedTypesArentSupported);
        };

        let allow_null = match self.peek_token() {
            Some(Token::QuestionMark) => {
                self.consume_token();
                true
            }
            _ => false,
        };

        self.push_node(Node::Type {
            type_index,
            allow_null,
        })
    }

    // Helper for parse_function() that recursively parses nested function arguments
    // e.g.
    //   f = |(foo, bar, (x, y))|
    //   #     ^ You are here
    //   #                ^ ...or here
    fn parse_nested_function_args(
        &mut self,
        arg_ids: &mut AstVec<ConstantIndex>,
    ) -> Result<AstVec<AstIndex>> {
        let mut nested_args = AstVec::new();

        let args_context = ExpressionContext::inside_braces();
        while self.peek_token_with_context(&args_context).is_some() {
            self.consume_until_token_with_context(&args_context);
            match self.parse_id_or_wildcard(&args_context)? {
                Some(IdOrWildcard::Id(constant_index)) => {
                    if self.constants.get_str(constant_index) == "self" {
                        return self.error(SyntaxError::SelfArg);
                    }

                    let arg_span = self.current_span();
                    let arg_node = if self.peek_token() == Some(Token::Ellipsis) {
                        self.consume_token();
                        Node::PackedId(Some(constant_index))
                    } else {
                        Node::Id(constant_index, self.parse_type_hint(&args_context)?)
                    };

                    nested_args.push(self.push_node_with_span(arg_node, arg_span)?);
                    arg_ids.push(constant_index);
                }
                Some(IdOrWildcard::Wildcard(maybe_id)) => {
                    let arg_span = self.current_span();
                    let type_hint = self.parse_type_hint(&args_context)?;
                    nested_args.push(
                        self.push_node_with_span(Node::Wildcard(maybe_id, type_hint), arg_span)?,
                    );
                }
                None => match self.peek_token() {
                    Some(Token::RoundOpen) => {
                        self.consume_token();
                        let span_start = self.current_span();

                        let tuple_args = self.parse_nested_function_args(arg_ids)?;
                        if !matches!(
                            self.consume_token_with_context(&args_context),
                            Some((Token::RoundClose, _))
                        ) {
                            return self.error(SyntaxError::ExpectedCloseParen);
                        }
                        nested_args.push(
                            self.push_node_with_start_span(Node::Tuple(tuple_args), span_start)?,
                        );
                    }
                    Some(Token::Ellipsis) => {
                        self.consume_token();
                        nested_args.push(self.push_node(Node::PackedId(None))?);
                    }
                    _ => break,
                },
            }

            if self.peek_next_token_on_same_line() == Some(Token::Comma) {
                self.consume_next_token_on_same_line();
            } else {
                break;
            }
        }

        Ok(nested_args)
    }

    // Attempts to parse whitespace-separated call args
    //
    // The context is used to determine what kind of argument separation is allowed.
    //
    // The resulting Vec will be empty if no arguments were encountered.
    //
    // See also parse_parenthesized_args.
    fn parse_call_args(&mut self, context: &ExpressionContext) -> Result<AstVec<AstIndex>> {
        let mut args = AstVec::new();

        if context.allow_space_separated_call {
            let mut arg_context = ExpressionContext {
                expected_indentation: Indentation::Greater,
                ..*context
            };

            let mut last_arg_line = self.current_line;

            while let Some(peeked) = self.peek_token_with_context(&arg_context) {
                let new_line = peeked.info.line() > last_arg_line;
                last_arg_line = peeked.info.line();

                if new_line {
                    arg_context.expected_indentation = Indentation::Equal(peeked.info.indent);
                } else if args.is_empty() && self.peek_token() != Some(Token::Whitespace) {
                    break;
                }

                let arg_start_span = self.current_span();
                if let Some(expression) = self
                    .parse_expression_with_min_precedence(MIN_PRECEDENCE_AFTER_PIPE, &arg_context)?
                {
                    let arg_expression = if self.peek_token() == Some(Token::Ellipsis) {
                        self.consume_token();
                        self.push_node_with_start_span(
                            Node::PackedExpression(expression),
                            arg_start_span,
                        )?
                    } else {
                        expression
                    };

                    args.push(arg_expression);
                } else {
                    break;
                }

                if self.peek_next_token_on_same_line() == Some(Token::Comma)
                    && !context.inside_braces
                {
                    self.consume_next_token_on_same_line();
                } else {
                    break;
                }
            }
        }

        Ok(args)
    }

    // Parses a single id
    //
    // See also: parse_id_or_wildcard(), consume_id_expression()
    fn parse_id(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<(ConstantIndex, ExpressionContext)>> {
        match self.peek_token_with_context(context) {
            Some(PeekInfo {
                token: Token::Id, ..
            }) => {
                let (_, id_context) = self.consume_token_with_context(context).unwrap();
                let constant_index = self.add_current_slice_as_string_constant()?;
                Ok(Some((constant_index, id_context)))
            }
            _ => Ok(None),
        }
    }

    // Consumes a single `_` wildcard, along with its optional following id
    fn consume_wildcard(&mut self, context: &ExpressionContext) -> Result<Option<ConstantIndex>> {
        if !matches!(
            self.consume_token_with_context(context),
            Some((Token::Wildcard, _))
        ) {
            return self.error(InternalError::UnexpectedToken);
        }

        let slice = self.current_token.slice(self.source);
        let maybe_id = if slice.len() > 1 {
            Some(self.add_string_constant(&slice[1..])?)
        } else {
            None
        };

        Ok(maybe_id)
    }

    // Parses either an id or a wildcard
    //
    // Used in function arguments, match expressions, etc.
    fn parse_id_or_wildcard(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<IdOrWildcard>> {
        match self.peek_token_with_context(context) {
            Some(PeekInfo {
                token: Token::Id, ..
            }) => {
                self.consume_token_with_context(context);
                self.add_current_slice_as_string_constant()
                    .map(|result| Some(IdOrWildcard::Id(result)))
            }
            Some(PeekInfo {
                token: Token::Wildcard,
                ..
            }) => {
                let maybe_id = self.consume_wildcard(context)?;
                Ok(Some(IdOrWildcard::Wildcard(maybe_id)))
            }
            _ => Ok(None),
        }
    }

    fn parse_id_or_string(&mut self, context: &ExpressionContext) -> Result<Option<AstIndex>> {
        let result = match self.parse_id(context)? {
            Some((id, _)) => Some(self.push_node(Node::Id(id, None))?),
            None => match self.parse_string(context)? {
                Some(s) => Some(self.push_node_with_span(Node::Str(s.string), s.span)?),
                None => None,
            },
        };

        Ok(result)
    }

    fn consume_id_expression(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        let Some((constant_index, id_context)) = self.parse_id(context)? else {
            return self.consume_token_and_error(InternalError::UnexpectedToken);
        };

        let id_span = self.current_span();

        let id_node = self.push_node(Node::Id(constant_index, None))?;

        if id_context.allow_map_block && self.peek_next_token_on_same_line() == Some(Token::Colon) {
            // The ID is the start of a map block
            if context.export_map_entries {
                self.frame_mut()?.add_local_id_assignment(constant_index);
            }
            self.consume_map_block(id_node, id_span, &id_context)
        } else {
            self.frame_mut()?.add_id_access(constant_index);

            let chain_context = id_context.chain_start();
            if self.next_token_is_chain_start(&chain_context) {
                // The ID is the start of a chain
                self.consume_chain(id_node, &chain_context)
            } else {
                // Check for paren-free call args following the ID
                let args = self.parse_call_args(&id_context)?;

                if args.is_empty() {
                    // No args, so this is a plain ID access
                    Ok(id_node)
                } else {
                    // Args were found, so add them to a chained call
                    let call_node = self.push_node_with_start_span(
                        Node::Chain((
                            ChainNode::Call {
                                args,
                                with_parens: false,
                            },
                            None,
                        )),
                        id_span,
                    )?;
                    self.push_node_with_start_span(
                        Node::Chain((ChainNode::Root(id_node), Some(call_node))),
                        id_span,
                    )
                }
            }
        }
    }

    fn consume_self_expression(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        let Some((_, self_context)) = self.consume_token_with_context(context) else {
            return self.error(SyntaxError::ExpectedCloseParen);
        };

        let chain_context = self_context.chain_start();
        let self_index = self.push_node(Node::Self_)?;

        if self.next_token_is_chain_start(&chain_context) {
            self.consume_chain(self_index, &chain_context)
        } else {
            Ok(self_index)
        }
    }

    // Checks to see if a chain starts after the parsed node,
    // and either returns the node if there's no chain,
    // or uses the node as the start of the chain.
    fn check_for_chain_after_node(
        &mut self,
        node: AstIndex,
        context: &ExpressionContext,
    ) -> Result<AstIndex> {
        let chain_context = context.chain_start();
        if self.next_token_is_chain_start(&chain_context) {
            self.consume_chain(node, &chain_context)
        } else {
            Ok(node)
        }
    }

    // Returns true if the following token is the start of a chain
    //
    // If the following token is on the same line, then it must be the _next_ token,
    // otherwise the context is used to find an indented token on a following line.
    fn next_token_is_chain_start(&mut self, context: &ExpressionContext) -> bool {
        use Token::*;

        if matches!(
            self.peek_token(),
            Some(Dot | SquareOpen | RoundOpen | QuestionMark)
        ) {
            true
        } else if context.allow_linebreaks {
            matches!(
                self.peek_token_with_context(context),
                Some(peeked) if peeked.token == Dot
            )
        } else {
            false
        }
    }

    // Parses an expression chain
    //
    // Expression chains represent a series of map accesses, index operations, and function calls.
    //
    // The root of the chain (i.e. the initial expression that is followed by `.`, `[`, or `(`)
    // has already been parsed and is passed in as the `root` argument.
    //
    // e.g.
    //   foo.bar()
    //   #  ^ You are here
    //
    // e.g.
    //   y = x[0][1].foo()
    //   #    ^ You are here
    fn consume_chain(&mut self, root: AstIndex, context: &ExpressionContext) -> Result<AstIndex> {
        let mut chain = AstVec::new();
        let mut chain_line = self.current_line;

        let mut node_context = *context;
        let mut node_start_span = self.current_span();
        let restricted = ExpressionContext::restricted();

        chain.push((ChainNode::Root(root), node_start_span));

        while let Some(token) = self.peek_token() {
            match token {
                // Function call
                Token::RoundOpen => {
                    self.consume_token();

                    let args = self.parse_parenthesized_args()?;

                    chain.push((
                        ChainNode::Call {
                            args,
                            with_parens: true,
                        },
                        node_start_span,
                    ));
                }
                // Index
                Token::SquareOpen => {
                    self.consume_token();

                    let index_expression = self.consume_index_expression()?;

                    if let Some(Token::SquareClose) = self.consume_next_token_on_same_line() {
                        chain.push((ChainNode::Index(index_expression), node_start_span));
                    } else {
                        return self.error(SyntaxError::ExpectedIndexEnd);
                    }
                }
                // Map access
                Token::Dot => {
                    self.consume_token();

                    if !matches!(
                        self.peek_token(),
                        Some(Token::Id | Token::StringStart { .. })
                    ) {
                        // This check prevents detached dot accesses, e.g. `x. foo`
                        return self.consume_token_and_error(SyntaxError::ExpectedMapKey);
                    } else if let Some((id, _)) = self.parse_id(&restricted)? {
                        node_start_span = self.current_span();
                        chain.push((ChainNode::Id(id), node_start_span));
                    } else if let Some(access_string) = self.parse_string(&restricted)? {
                        node_start_span = access_string.span;
                        chain.push((ChainNode::Str(access_string.string), access_string.span));
                    } else {
                        return self.consume_token_and_error(SyntaxError::ExpectedMapKey);
                    }
                }
                // Null check
                Token::QuestionMark => {
                    self.consume_token();
                    if matches!(chain.last(), Some((ChainNode::NullCheck, _))) {
                        return self.error(SyntaxError::UnexpectedNullCheck);
                    }

                    chain.push((ChainNode::NullCheck, node_start_span));
                }
                _ => {
                    let Some(peeked) = self.peek_token_with_context(&node_context) else {
                        break;
                    };
                    if peeked.token == Token::Dot {
                        // Indented Dot on a following line?

                        // Consume up until the Dot,
                        // which will be picked up on the next iteration
                        node_context = self
                            .consume_until_token_with_context(&node_context)
                            .unwrap();

                        // Check that the next dot is on an indented line
                        if self.current_line == chain_line {
                            return self.consume_token_and_error(SyntaxError::ExpectedMapKey);
                        }

                        // Starting a new line, so space separated calls are allowed
                        node_context.allow_space_separated_call = true;
                    } else {
                        // Attempt to parse trailing call arguments,
                        // e.g.
                        //   x.foo 42, 99
                        //         ~~~~~~
                        //
                        //   x.foo
                        //     42, 99
                        //     ~~~~~~
                        //
                        //   foo
                        //     .bar 123
                        //          ~~~
                        //     .baz
                        //       x, y
                        //       ~~~~
                        //
                        //   foo.takes_a_map_arg
                        //     bar: 42
                        //     ~~~~~~~

                        // Allow a map block if we're on an indented line
                        node_context.allow_map_block = peeked.info.line() > chain_line;

                        let args = self.parse_call_args(&node_context)?;

                        // Now that space separated args have been parsed,
                        // don't allow any more while we're on the same line.
                        node_context.allow_space_separated_call = false;

                        if args.is_empty() {
                            // No arguments found, so we're at the end of the chain
                            break;
                        } else {
                            chain.push((
                                ChainNode::Call {
                                    args,
                                    with_parens: false,
                                },
                                node_start_span,
                            ));
                        }
                    }

                    chain_line = self.current_line;
                }
            }
        }

        // Add the chain nodes to the AST in reverse order:
        // the final AST index will be the chain root node.
        let mut next_index = None;
        for (node, span) in chain.iter().rev() {
            next_index =
                Some(self.push_node_with_span(Node::Chain((node.clone(), next_index)), *span)?);
        }
        next_index.ok_or_else(|| self.make_error(InternalError::ChainParseFailure))
    }

    // Helper for consume_chain() that parses an index expression
    //
    // e.g.
    //   foo.bar[10..20]
    //   #       ^ You are here
    fn consume_index_expression(&mut self) -> Result<AstIndex> {
        let index_context = ExpressionContext::restricted();

        let result = if let Some(index_expression) = self.parse_expression(&index_context)? {
            match self.peek_token() {
                Some(Token::Range) => {
                    self.consume_token();

                    if let Some(end_expression) = self.parse_expression(&index_context)? {
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

                    if let Some(end_expression) = self.parse_expression(&index_context)? {
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
            match self.consume_next_token_on_same_line() {
                Some(Token::Range) => {
                    if let Some(end_expression) = self.parse_expression(&index_context)? {
                        self.push_node(Node::RangeTo {
                            end: end_expression,
                            inclusive: false,
                        })?
                    } else {
                        self.push_node(Node::RangeFull)?
                    }
                }
                Some(Token::RangeInclusive) => {
                    if let Some(end_expression) = self.parse_expression(&index_context)? {
                        self.push_node(Node::RangeTo {
                            end: end_expression,
                            inclusive: true,
                        })?
                    } else {
                        self.push_node(Node::RangeFull)?
                    }
                }
                _ => return self.error(SyntaxError::ExpectedIndexExpression),
            }
        };

        Ok(result)
    }

    // Helper for consume_chain() that parses the args in a chained function call
    //
    // e.g.
    // foo[0].bar(1, 2, 3)
    // #          ^ You are here
    fn parse_parenthesized_args(&mut self) -> Result<AstVec<AstIndex>> {
        let start_indent = self.current_indent();
        let mut args = AstVec::new();
        let mut args_context = ExpressionContext::permissive();

        while self.peek_token_with_context(&args_context).is_some() {
            args_context = self
                .consume_until_token_with_context(&args_context)
                .unwrap();

            let arg_start_span = self.current_span();
            if let Some(expression) = self.parse_expression(&ExpressionContext::inside_braces())? {
                let arg_expression = if self.peek_token() == Some(Token::Ellipsis) {
                    self.consume_token();
                    self.push_node_with_start_span(
                        Node::PackedExpression(expression),
                        arg_start_span,
                    )?
                } else {
                    expression
                };

                args.push(arg_expression);
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
        args_end_context.expected_indentation = Indentation::Equal(start_indent);
        if !matches!(
            self.consume_token_with_context(&args_end_context),
            Some((Token::RoundClose, _))
        ) {
            return self.error(SyntaxError::ExpectedArgsEnd);
        }

        Ok(args)
    }

    fn consume_range(
        &mut self,
        lhs: Option<AstIndex>,
        context: &ExpressionContext,
    ) -> Result<AstIndex> {
        use Node::{Range, RangeFrom, RangeFull, RangeTo};

        let inclusive = match self.consume_next_token_on_same_line() {
            Some(Token::Range) => false,
            Some(Token::RangeInclusive) => true,
            _ => return self.error(InternalError::UnexpectedToken),
        };

        let mut start_span = self.current_span();

        if lhs.is_none() {
            // e.g.
            // for x in ..10
            //          ^^ <- we want the span to start here if we don't have a LHS
            start_span = self.current_span();
        }

        let rhs = self.parse_expression(&ExpressionContext::inline())?;

        let range_node = match (lhs, rhs) {
            (Some(start), Some(end)) => Range {
                start,
                end,
                inclusive,
            },
            (Some(start), None) => RangeFrom { start },
            (None, Some(end)) => RangeTo { end, inclusive },
            (None, None) => RangeFull,
        };

        let range_node = self.push_node_with_start_span(range_node, start_span)?;
        self.check_for_chain_after_node(range_node, context)
    }

    fn consume_export(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::Export
        let start_span = self.current_span();

        if let Some(expression) = self.parse_expressions(
            &ExpressionContext::permissive().with_exported_map_entries(),
            TempResult::No,
        )? {
            self.push_node_with_start_span(Node::Export(expression), start_span)
        } else {
            self.consume_token_and_error(SyntaxError::ExpectedExpression)
        }
    }

    fn consume_throw_expression(&mut self) -> Result<AstIndex> {
        self.consume_next_token_on_same_line(); // Token::Throw
        let start_span = self.current_span();

        if let Some(expression) = self.parse_expression(&ExpressionContext::permissive())? {
            self.push_node_with_start_span(Node::Throw(expression), start_span)
        } else {
            self.consume_token_and_error(SyntaxError::ExpectedExpression)
        }
    }

    fn consume_debug_expression(&mut self) -> Result<AstIndex> {
        self.consume_next_token_on_same_line(); // Token::Debug

        let start_position = self.current_span().start;

        let context = ExpressionContext::permissive();
        let Some(expression_start_info) = self.peek_token_with_context(&context) else {
            return self.consume_token_and_error(SyntaxError::ExpectedExpression);
        };
        let expression_source_start = expression_start_info.info.source_bytes.start;

        let Some(expression) = self.parse_expressions(&context, TempResult::No)? else {
            return self.consume_token_and_error(SyntaxError::ExpectedExpression);
        };

        let expression_source_end = self.current_token.source_bytes.end;

        let expression_string =
            self.add_string_constant(&self.source[expression_source_start..expression_source_end])?;

        self.ast.push(
            Node::Debug {
                expression_string,
                expression,
            },
            Span {
                start: start_position,
                end: self.current_span().end,
            },
        )
    }

    fn consume_number(&mut self, negate: bool, context: &ExpressionContext) -> Result<AstIndex> {
        use Node::*;

        self.consume_token_with_context(context); // Token::Number

        let slice = self.current_token.slice(self.source);
        // Strip underscores if necessary
        let slice = if slice.contains('_') {
            Cow::Owned(slice.chars().filter(|&c| c != '_').collect())
        } else {
            Cow::Borrowed(slice)
        };

        let maybe_integer = if let Some(hex) = slice.strip_prefix("0x") {
            i64::from_str_radix(hex, 16)
        } else if let Some(octal) = slice.strip_prefix("0o") {
            i64::from_str_radix(octal, 8)
        } else if let Some(binary) = slice.strip_prefix("0b") {
            i64::from_str_radix(binary, 2)
        } else {
            i64::from_str(&slice)
        };

        let number_node = if let Ok(n) = maybe_integer {
            // Should we store the number as a SmallInt or as a stored constant?
            if u8::try_from(n).is_ok() {
                let n = if negate { -n } else { n };
                SmallInt(n as i16)
            } else {
                let n = if negate { -n } else { n };
                if let Ok(constant_index) = self.constants.add_i64(n) {
                    Int(constant_index)
                } else {
                    return self.error(InternalError::ConstantPoolCapacityOverflow);
                }
            }
        } else if let Ok(n) = f64::from_str(&slice) {
            let n = if negate { -n } else { n };
            if let Ok(constant_index) = self.constants.add_f64(n) {
                Float(constant_index)
            } else {
                return self.error(InternalError::ConstantPoolCapacityOverflow);
            }
        } else {
            return self.error(InternalError::NumberParseFailure);
        };

        let node = self.push_node(number_node)?;
        self.check_for_chain_after_node(node, context)
    }

    // Parses expressions contained in round parentheses
    // The result may be:
    //   - Null
    //     - e.g. `()`
    //   - A single expression
    //     - e.g. `(1 + 1)`
    //   - A comma-separated tuple
    //     - e.g. `(,)`, `(x,)`, `(1, 2)`
    fn consume_tuple(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::RoundOpen

        let start_span = self.current_span();
        let start_indent = self.current_indent();

        let (entries, last_token_was_a_comma) =
            self.parse_comma_separated_entries(Token::RoundClose)?;
        self.expect_and_consume_token(
            Token::RoundClose,
            SyntaxError::ExpectedCloseParen.into(),
            &ExpressionContext::inside_braces(),
        )?;

        let expressions_node = match entries.as_slice() {
            [single_expression] if !last_token_was_a_comma => {
                self.push_node_with_start_span(Node::Nested(*single_expression), start_span)?
            }
            _ => self.push_node_with_start_span(Node::Tuple(entries), start_span)?,
        };

        self.check_for_chain_after_node(
            expressions_node,
            &context.with_expected_indentation(Indentation::GreaterThan(start_indent)),
        )
    }

    // Parses a list, e.g. `[1, 2, 3]`
    fn consume_list(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::SquareOpen

        let start_span = self.current_span();
        let start_indent = self.current_indent();

        let (entries, _) = self.parse_comma_separated_entries(Token::SquareClose)?;
        self.expect_and_consume_token(
            Token::SquareClose,
            SyntaxError::ExpectedListEnd.into(),
            &ExpressionContext::inside_braces(),
        )?;

        let list_node = self.push_node_with_start_span(Node::List(entries), start_span)?;
        self.check_for_chain_after_node(
            list_node,
            &context.with_expected_indentation(Indentation::GreaterThan(start_indent)),
        )
    }

    // Helper for parse_list and parse_tuple
    //
    // Returns a Vec of entries along with a bool that's true if the last token before the end
    // was a comma, which is used by parse_tuple to determine how the entries should be
    // parsed.
    fn parse_comma_separated_entries(
        &mut self,
        end_token: Token,
    ) -> Result<(AstVec<AstIndex>, bool)> {
        let mut entries = AstVec::new();
        let entry_context = ExpressionContext::inside_braces();
        let mut last_token_was_a_comma = false;

        while matches!(
            self.peek_token_with_context(&entry_context),
            Some(peeked) if peeked.token != end_token)
        {
            self.consume_until_token_with_context(&entry_context);

            if let Some(entry) = self.parse_expression(&entry_context)? {
                entries.push(entry);
                last_token_was_a_comma = false;
            }

            if matches!(
                self.peek_token_with_context(&entry_context),
                Some(PeekInfo {
                    token: Token::Comma,
                    ..
                })
            ) {
                self.consume_token_with_context(&entry_context);

                if entries.is_empty() || last_token_was_a_comma {
                    entries.push(self.push_node(Node::Null)?);
                }

                last_token_was_a_comma = true;
            } else {
                break;
            }
        }

        Ok((entries, last_token_was_a_comma))
    }

    fn consume_map_block(
        &mut self,
        first_key: AstIndex,
        start_span: Span,
        context: &ExpressionContext,
    ) -> Result<AstIndex> {
        if !context.allow_map_block {
            return self.error(SyntaxError::ExpectedLineBreakBeforeMapBlock);
        }

        let start_indent = self.current_indent();

        if self.consume_next_token_on_same_line() != Some(Token::Colon) {
            return self.error(InternalError::ExpectedMapColon);
        }

        let mut entries = astvec![(first_key, Some(self.consume_map_block_value()?))];

        let block_context = ExpressionContext::permissive()
            .with_expected_indentation(Indentation::Equal(start_indent));

        while self.peek_token_with_context(&block_context).is_some() {
            self.consume_until_token_with_context(&block_context);

            let Some(key) = self.parse_map_key(context.export_map_entries)? else {
                return self.consume_token_and_error(SyntaxError::ExpectedMapEntry);
            };

            if self.peek_next_token_on_same_line() != Some(Token::Colon) {
                return self.consume_token_and_error(SyntaxError::ExpectedMapColon);
            };

            self.consume_next_token_on_same_line(); // ':'

            entries.push((key, Some(self.consume_map_block_value()?)));
        }

        self.push_node_with_start_span(Node::Map(entries), start_span)
    }

    fn consume_map_block_value(&mut self) -> Result<AstIndex> {
        if let Some(value) = self.parse_indented_block()? {
            Ok(value)
        } else if let Some(value) = self.parse_line(&ExpressionContext::permissive())? {
            Ok(value)
        } else {
            self.consume_token_and_error(SyntaxError::ExpectedMapValue)
        }
    }

    fn consume_map_with_braces(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::CurlyOpen

        let start_indent = self.current_indent();
        let start_span = self.current_span();

        let entries = self.parse_comma_separated_map_entries(context)?;
        self.expect_and_consume_token(
            Token::CurlyClose,
            SyntaxError::ExpectedMapEnd.into(),
            &ExpressionContext::inside_braces(),
        )?;

        let map_node = self.push_node_with_start_span(Node::Map(entries), start_span)?;
        self.check_for_chain_after_node(
            map_node,
            &context.with_expected_indentation(Indentation::GreaterThan(start_indent)),
        )
    }

    fn parse_comma_separated_map_entries(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<AstVec<(AstIndex, Option<AstIndex>)>> {
        let mut entries = AstVec::new();
        let mut entry_context = ExpressionContext::inside_braces();

        while self.peek_token_with_context(&entry_context).is_some() {
            self.consume_until_token_with_context(&entry_context);

            let Some(key) = self.parse_map_key(context.export_map_entries)? else {
                break;
            };

            if self.peek_next_token_on_same_line() == Some(Token::Colon) {
                self.consume_next_token_on_same_line();

                if self.peek_token_with_context(&entry_context).is_none() {
                    return self.error(SyntaxError::ExpectedMapValue);
                }
                self.consume_until_token_with_context(&entry_context);

                if let Some(value) = self.parse_expression(&entry_context)? {
                    entries.push((key, Some(value)));
                } else {
                    return self.consume_token_and_error(SyntaxError::ExpectedMapValue);
                }
            } else {
                // Valueless map entries are allowed in inline maps,
                // e.g.
                //   bar = -1
                //   x = {foo: 42, bar, baz: 99}
                match self.ast.node(key).node {
                    Node::Id(id, ..) => self.frame_mut()?.add_id_access(id),
                    _ => return self.error(SyntaxError::ExpectedMapValue),
                }
                entries.push((key, None));
            }

            if matches!(
                self.peek_token_with_context(&entry_context),
                Some(PeekInfo {
                    token: Token::Comma,
                    ..
                })
            ) {
                self.consume_token_with_context(&entry_context);
                entry_context = ExpressionContext::inside_braces();
            } else {
                break;
            }
        }

        Ok(entries)
    }

    // Helper for map parsing, attempts to parse a map key from the current position
    //
    // Map keys come in three flavours, e.g.:
    //   my_map =
    //     regular_id: 1
    //     'string_id': 2
    //     @meta meta_key: 3
    fn parse_map_key(&mut self, export_map_entry: bool) -> Result<Option<AstIndex>> {
        let result = if let Some((id, _)) = self.parse_id(&ExpressionContext::restricted())? {
            if export_map_entry {
                self.frame_mut()?.add_local_id_assignment(id);
            }
            Some(self.push_node(Node::Id(id, None))?)
        } else if let Some(s) = self.parse_string(&ExpressionContext::restricted())? {
            Some(self.push_node_with_span(Node::Str(s.string), s.span)?)
        } else {
            self.parse_meta_key()?
        };

        Ok(result)
    }

    // Attempts to parse a meta key
    fn parse_meta_key(&mut self) -> Result<Option<AstIndex>> {
        if self.peek_next_token_on_same_line() != Some(Token::At) {
            return Ok(None);
        }

        self.consume_next_token_on_same_line();
        let start_span = self.current_span();

        let mut meta_name = None;

        let meta_key_id = match self.consume_token() {
            Some(Token::Add) => MetaKeyId::Add,
            Some(Token::Subtract) => MetaKeyId::Subtract,
            Some(Token::Multiply) => MetaKeyId::Multiply,
            Some(Token::Divide) => MetaKeyId::Divide,
            Some(Token::Remainder) => MetaKeyId::Remainder,
            Some(Token::Power) => MetaKeyId::Power,
            Some(Token::AddAssign) => MetaKeyId::AddAssign,
            Some(Token::SubtractAssign) => MetaKeyId::SubtractAssign,
            Some(Token::MultiplyAssign) => MetaKeyId::MultiplyAssign,
            Some(Token::DivideAssign) => MetaKeyId::DivideAssign,
            Some(Token::RemainderAssign) => MetaKeyId::RemainderAssign,
            Some(Token::PowerAssign) => MetaKeyId::PowerAssign,
            Some(Token::Less) => MetaKeyId::Less,
            Some(Token::LessOrEqual) => MetaKeyId::LessOrEqual,
            Some(Token::Greater) => MetaKeyId::Greater,
            Some(Token::GreaterOrEqual) => MetaKeyId::GreaterOrEqual,
            Some(Token::Equal) => MetaKeyId::Equal,
            Some(Token::NotEqual) => MetaKeyId::NotEqual,
            Some(Token::Debug) => MetaKeyId::Debug,
            Some(Token::Id) => match self.current_token.slice(self.source) {
                "r" => match self.consume_next_token_on_same_line() {
                    Some(Token::Add) => MetaKeyId::AddRhs,
                    Some(Token::Subtract) => MetaKeyId::SubtractRhs,
                    Some(Token::Multiply) => MetaKeyId::MultiplyRhs,
                    Some(Token::Divide) => MetaKeyId::DivideRhs,
                    Some(Token::Remainder) => MetaKeyId::RemainderRhs,
                    Some(Token::Power) => MetaKeyId::PowerRhs,
                    _ => return self.error(SyntaxError::UnexpectedMetaKey),
                },
                "call" => MetaKeyId::Call,
                "display" => MetaKeyId::Display,
                "index" => MetaKeyId::Index,
                "index_mut" => MetaKeyId::IndexMut,
                "iterator" => MetaKeyId::Iterator,
                "next" => MetaKeyId::Next,
                "next_back" => MetaKeyId::NextBack,
                "negate" => MetaKeyId::Negate,
                "size" => MetaKeyId::Size,
                "type" => MetaKeyId::Type,
                "base" => MetaKeyId::Base,
                "main" => MetaKeyId::Main,
                "pre_test" => MetaKeyId::PreTest,
                "post_test" => MetaKeyId::PostTest,
                "test" => match self.consume_next_token_on_same_line() {
                    Some(Token::Id) => {
                        let test_name = self.add_current_slice_as_string_constant()?;
                        meta_name = Some(test_name);
                        MetaKeyId::Test
                    }
                    _ => return self.error(SyntaxError::ExpectedTestName),
                },
                "meta" => match self.consume_next_token_on_same_line() {
                    Some(Token::Id) => {
                        let id = self.add_current_slice_as_string_constant()?;
                        meta_name = Some(id);
                        MetaKeyId::Named
                    }
                    _ => return self.error(SyntaxError::ExpectedMetaId),
                },
                _ => return self.error(SyntaxError::UnexpectedMetaKey),
            },
            _ => return self.error(SyntaxError::UnexpectedMetaKey),
        };

        let result =
            self.push_node_with_start_span(Node::Meta(meta_key_id, meta_name), start_span)?;

        Ok(Some(result))
    }

    fn consume_for_loop(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::For

        let start_span = self.current_span();

        let mut args = AstVec::new();
        while let Some(id_or_wildcard) = self.parse_id_or_wildcard(context)? {
            let arg_span = self.current_span();
            let type_hint = self.parse_type_hint(context)?;

            let arg_node = match id_or_wildcard {
                IdOrWildcard::Id(id) => {
                    self.frame_mut()?.ids_assigned_in_frame.insert(id);
                    Node::Id(id, type_hint)
                }
                IdOrWildcard::Wildcard(maybe_id) => Node::Wildcard(maybe_id, type_hint),
            };
            args.push(self.push_node_with_span(arg_node, arg_span)?);

            match self.peek_next_token_on_same_line() {
                Some(Token::Comma) => {
                    self.consume_next_token_on_same_line();
                }
                Some(Token::In) => {
                    self.consume_next_token_on_same_line();
                    break;
                }
                _ => return self.consume_token_and_error(SyntaxError::ExpectedForInKeyword),
            }
        }
        if args.is_empty() {
            return self.consume_token_and_error(SyntaxError::ExpectedForArgs);
        }

        let iterable = match self.parse_expression(&ExpressionContext::inline())? {
            Some(iterable) => iterable,
            None => return self.consume_token_and_error(SyntaxError::ExpectedForIterable),
        };

        match self.parse_indented_block()? {
            Some(body) => {
                let result = self.push_node_with_start_span(
                    Node::For(AstFor {
                        args,
                        iterable,
                        body,
                    }),
                    start_span,
                )?;

                Ok(result)
            }
            None => self.consume_token_and_error(ExpectedIndentation::ForBody),
        }
    }

    // Parses a loop declared with the `loop` keyword
    fn consume_loop_block(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::Loop

        if let Some(body) = self.parse_indented_block()? {
            self.push_node(Node::Loop { body })
        } else {
            self.consume_token_and_error(ExpectedIndentation::LoopBody)
        }
    }

    fn consume_while_loop(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::While

        let Some(condition) = self.parse_expression(&ExpressionContext::inline())? else {
            return self.consume_token_and_error(SyntaxError::ExpectedWhileCondition);
        };

        match self.parse_indented_block()? {
            Some(body) => self.push_node(Node::While { condition, body }),
            None => self.consume_token_and_error(ExpectedIndentation::WhileBody),
        }
    }

    fn consume_until_loop(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::Until

        let Some(condition) = self.parse_expression(&ExpressionContext::inline())? else {
            return self.consume_token_and_error(SyntaxError::ExpectedUntilCondition);
        };

        match self.parse_indented_block()? {
            Some(body) => self.push_node(Node::Until { condition, body }),
            None => self.consume_token_and_error(ExpectedIndentation::UntilBody),
        }
    }

    fn consume_if_expression(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        use SyntaxError::*;

        self.consume_token_with_context(context); // Token::If

        let if_span = self.current_span();

        // Define the expected indentation of 'else if' / 'else' blocks
        let mut outer_context =
            context.with_expected_indentation(Indentation::GreaterOrEqual(self.current_indent()));

        let Some(condition) = self.parse_expression(&ExpressionContext::inline())? else {
            return self.consume_token_and_error(ExpectedIfCondition);
        };

        if self.peek_next_token_on_same_line() == Some(Token::Then) {
            self.consume_next_token_on_same_line();
            let Some(then_node) =
                self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
            else {
                return self.error(ExpectedThenExpression);
            };

            let else_node = if self.peek_next_token_on_same_line() == Some(Token::Else) {
                self.consume_next_token_on_same_line();
                match self.parse_expressions(&ExpressionContext::inline(), TempResult::No)? {
                    Some(else_node) => Some(else_node),
                    None => return self.error(ExpectedElseExpression),
                }
            } else {
                None
            };

            self.push_node_with_span(
                Node::If(AstIf {
                    condition,
                    then_node,
                    else_if_blocks: astvec![],
                    else_node,
                }),
                if_span,
            )
        } else {
            if !outer_context.allow_linebreaks {
                return self.error(IfBlockNotAllowedInThisContext);
            }

            if let Some(then_node) = self.parse_indented_block()? {
                let mut else_if_blocks = AstVec::new();

                while let Some(peeked) = self.peek_token_with_context(&outer_context) {
                    if peeked.token != Token::ElseIf {
                        break;
                    }

                    self.consume_token_with_context(&outer_context);

                    // Once we've got an else if block, then all following blocks in the
                    // cascade should start with the same indentation.
                    outer_context = context
                        .with_expected_indentation(Indentation::Equal(self.current_indent()));

                    let Some(else_if_condition) =
                        self.parse_expression(&ExpressionContext::inline())?
                    else {
                        return self.consume_token_and_error(ExpectedElseIfCondition);
                    };

                    if let Some(else_if_block) = self.parse_indented_block()? {
                        else_if_blocks.push((else_if_condition, else_if_block));
                    } else {
                        return self.consume_token_on_same_line_and_error(
                            ExpectedIndentation::ElseIfBlock,
                        );
                    }
                }

                let else_node = match self.peek_token_with_context(&outer_context) {
                    Some(peeked) if peeked.token == Token::Else => {
                        self.consume_token_with_context(&outer_context);

                        if let Some(else_block) = self.parse_indented_block()? {
                            Some(else_block)
                        } else {
                            return self.consume_token_on_same_line_and_error(
                                ExpectedIndentation::ElseBlock,
                            );
                        }
                    }
                    _ => None,
                };

                self.push_node_with_span(
                    Node::If(AstIf {
                        condition,
                        then_node,
                        else_if_blocks,
                        else_node,
                    }),
                    if_span,
                )
            } else {
                self.consume_token_on_same_line_and_error(ExpectedIndentation::ThenKeywordOrBlock)
            }
        }
    }

    fn consume_switch_expression(
        &mut self,
        switch_context: &ExpressionContext,
    ) -> Result<AstIndex> {
        use SyntaxError::*;

        self.consume_token_with_context(switch_context); // Token::Switch

        let current_indent = self.current_indent();
        let switch_span = self.current_span();

        let arm_context = match self.consume_until_token_with_context(switch_context) {
            Some(arm_context) if self.current_indent() > current_indent => arm_context,
            _ => return self.consume_token_on_same_line_and_error(ExpectedIndentation::SwitchArm),
        };

        let mut arms = AstVec::new();

        while self.peek_token().is_some() {
            let condition = self.parse_expression(&ExpressionContext::inline())?;

            let arm_body = match self.peek_next_token_on_same_line() {
                Some(Token::Else) => {
                    if condition.is_some() {
                        return self.consume_token_and_error(UnexpectedSwitchElse);
                    }

                    self.consume_next_token_on_same_line();

                    if let Some(expression) =
                        self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
                    {
                        expression
                    } else if let Some(indented_block) = self.parse_indented_block()? {
                        indented_block
                    } else {
                        return self.consume_token_and_error(ExpectedSwitchArmExpression);
                    }
                }
                Some(Token::Then) => {
                    self.consume_next_token_on_same_line();

                    if let Some(expression) =
                        self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
                    {
                        expression
                    } else if let Some(indented_block) = self.parse_indented_block()? {
                        indented_block
                    } else {
                        return self.consume_token_and_error(ExpectedSwitchArmExpressionAfterThen);
                    }
                }
                _ => return self.consume_token_and_error(ExpectedSwitchArmExpression),
            };

            arms.push(SwitchArm {
                condition,
                expression: arm_body,
            });

            if self.peek_token_with_context(&arm_context).is_none() {
                break;
            }

            self.consume_until_token_with_context(&arm_context);
        }

        // Check for errors now that the match expression is complete
        for (arm_index, arm) in arms.iter().enumerate() {
            let last_arm = arm_index == arms.len() - 1;

            if arm.condition.is_none() && !last_arm {
                return Err(Error::new(SwitchElseNotInLastArm.into(), switch_span));
            }
        }

        self.push_node_with_span(Node::Switch(arms), switch_span)
    }

    fn consume_match_expression(&mut self, match_context: &ExpressionContext) -> Result<AstIndex> {
        use SyntaxError::*;

        self.consume_token_with_context(match_context); // Token::Match

        let current_indent = self.current_indent();
        let match_span = self.current_span();

        let match_expression =
            match self.parse_expressions(&ExpressionContext::inline(), TempResult::Yes)? {
                Some(expression) => expression,
                None => {
                    return self.consume_token_and_error(ExpectedMatchExpression);
                }
            };

        let arm_context = match self.consume_until_token_with_context(match_context) {
            Some(arm_context) if self.current_indent() > current_indent => arm_context,
            _ => return self.consume_token_on_same_line_and_error(ExpectedIndentation::MatchArm),
        };

        let mut arms = Vec::new();

        while self.peek_token().is_some() {
            // Match patterns for a single arm, with alternatives separated by 'or'
            // e.g. match x, y
            //   0, 1 then ...
            //   2, 3 or 4, 5 then ...
            //   other then ...
            let mut arm_patterns = AstVec::new();
            let mut expected_arm_count = 1;

            let condition = {
                while let Some(pattern) = self.parse_match_pattern(false)? {
                    // Match patterns, separated by commas in the case of matching multi-expressions
                    let mut patterns = astvec![pattern];

                    while let Some(Token::Comma) = self.peek_next_token_on_same_line() {
                        self.consume_next_token_on_same_line();

                        match self.parse_match_pattern(false)? {
                            Some(pattern) => patterns.push(pattern),
                            None => return self.consume_token_and_error(ExpectedMatchPattern),
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

                if self.peek_next_token_on_same_line() == Some(Token::If) {
                    self.consume_next_token_on_same_line();

                    match self.parse_expression(&ExpressionContext::inline())? {
                        Some(expression) => Some(expression),
                        None => return self.consume_token_and_error(ExpectedMatchCondition),
                    }
                } else {
                    None
                }
            };

            let arm_body = match self.peek_next_token_on_same_line() {
                Some(Token::Else) => {
                    if !arm_patterns.is_empty() || condition.is_some() {
                        return self.consume_token_and_error(UnexpectedMatchElse);
                    }

                    self.consume_next_token_on_same_line();

                    if let Some(expression) =
                        self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
                    {
                        expression
                    } else if let Some(indented_block) = self.parse_indented_block()? {
                        indented_block
                    } else {
                        return self.consume_token_and_error(ExpectedMatchArmExpression);
                    }
                }
                Some(Token::Then) => {
                    if arm_patterns.len() != expected_arm_count {
                        return self.consume_token_and_error(ExpectedMatchPattern);
                    }

                    self.consume_next_token_on_same_line();

                    if let Some(expression) =
                        self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
                    {
                        expression
                    } else if let Some(indented_block) = self.parse_indented_block()? {
                        indented_block
                    } else {
                        return self.consume_token_and_error(ExpectedMatchArmExpressionAfterThen);
                    }
                }
                Some(Token::If) => return self.consume_token_and_error(UnexpectedMatchIf),
                _ => return self.consume_token_and_error(ExpectedMatchArmExpression),
            };

            arms.push(MatchArm {
                patterns: arm_patterns,
                condition,
                expression: arm_body,
            });

            if self.peek_token_with_context(&arm_context).is_none() {
                break;
            }

            self.consume_until_token_with_context(&arm_context);
        }

        // Check for errors now that the match expression is complete

        for (arm_index, arm) in arms.iter().enumerate() {
            let last_arm = arm_index == arms.len() - 1;

            if arm.patterns.is_empty() && arm.condition.is_none() && !last_arm {
                return Err(Error::new(MatchElseNotInLastArm.into(), match_span));
            }
        }

        self.push_node_with_span(
            Node::Match {
                expression: match_expression,
                arms,
            },
            match_span,
        )
    }

    // Parses a match arm's pattern
    fn parse_match_pattern(&mut self, in_nested_patterns: bool) -> Result<Option<AstIndex>> {
        use Token::*;

        let pattern_context = ExpressionContext::restricted();

        let result = match self.peek_token_with_context(&pattern_context) {
            Some(peeked) => match peeked.token {
                True | False | Null | Number | StringStart { .. } | Subtract => {
                    return self.parse_term(&pattern_context);
                }
                Id => match self.parse_id(&pattern_context)? {
                    Some((id, _)) => {
                        let result = if self.peek_token() == Some(Ellipsis) {
                            self.consume_token();
                            if in_nested_patterns {
                                self.frame_mut()?.ids_assigned_in_frame.insert(id);
                                self.push_node(Node::PackedId(Some(id)))?
                            } else {
                                return self
                                    .error(SyntaxError::MatchEllipsisOutsideOfNestedPatterns);
                            }
                        } else {
                            let id_span = self.current_span();
                            let type_hint = self.parse_type_hint(&pattern_context)?;
                            let id_node =
                                self.push_node_with_span(Node::Id(id, type_hint), id_span)?;
                            if self.next_token_is_chain_start(&pattern_context) {
                                self.frame_mut()?.add_id_access(id);
                                self.consume_chain(id_node, &pattern_context)?
                            } else {
                                self.frame_mut()?.ids_assigned_in_frame.insert(id);
                                id_node
                            }
                        };
                        Some(result)
                    }
                    None => return self.error(InternalError::IdParseFailure),
                },
                Wildcard => {
                    let wildcard = self.consume_wildcard(&pattern_context)?;
                    let wildcard_span = self.current_span();
                    let maybe_type = self.parse_type_hint(&pattern_context)?;
                    let result = self
                        .push_node_with_span(Node::Wildcard(wildcard, maybe_type), wildcard_span)?;
                    Some(result)
                }
                RoundOpen => {
                    self.consume_token_with_context(&pattern_context);

                    if self.peek_token() == Some(RoundClose) {
                        self.consume_token();
                        Some(self.push_node(Node::Null)?)
                    } else {
                        let tuple_patterns = self.parse_nested_match_patterns()?;

                        if self.consume_next_token_on_same_line() != Some(RoundClose) {
                            return self.error(SyntaxError::ExpectedCloseParen);
                        }

                        Some(self.push_node(Node::Tuple(tuple_patterns))?)
                    }
                }
                Ellipsis if in_nested_patterns => {
                    self.consume_token_with_context(&pattern_context);
                    Some(self.push_node(Node::PackedId(None))?)
                }
                _ => None,
            },
            None => None,
        };

        Ok(result)
    }

    // Recursively parses nested match patterns
    //
    // e.g.
    //   match x
    //     (1, 2, (3, 4)) then ...
    //   #  ^ You are here
    //   #         ^...or here
    fn parse_nested_match_patterns(&mut self) -> Result<AstVec<AstIndex>> {
        let mut result = AstVec::new();

        while let Some(pattern) = self.parse_match_pattern(true)? {
            result.push(pattern);

            if self.peek_next_token_on_same_line() != Some(Token::Comma) {
                break;
            }
            self.consume_next_token_on_same_line();
        }

        Ok(result)
    }

    fn consume_import(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        let importing_from = match self.consume_token_with_context(context) {
            Some((Token::Import, _)) => false,
            Some((Token::From, _)) => true,
            _ => return self.error(InternalError::UnexpectedToken),
        };

        let start_span = self.current_span();
        let from_context = ExpressionContext::restricted();

        let from = if importing_from {
            // Parse the from module path: a nested path is allowed, but only a single path
            let from = self.consume_from_path(&from_context)?;

            match self.consume_token_with_context(&from_context) {
                Some((Token::Import, _)) => {}
                _ => return self.error(SyntaxError::ExpectedImportAfterFrom),
            }

            from
        } else {
            astvec![]
        };

        // Nested items aren't allowed, flatten the returned items into a single vec
        let items = self.consume_import_items(&ExpressionContext::permissive())?;

        // Mark any imported ids as locally assigned
        for item in items.iter() {
            let maybe_id = if let Node::Id(id, ..) = &self.ast.node(item.item).node {
                Some(*id)
            } else {
                None
            };
            let maybe_as =
                if let Some(Node::Id(id, ..)) = item.name.map(|node| &self.ast.node(node).node) {
                    Some(*id)
                } else {
                    None
                };
            if let (Some(id), None) | (_, Some(id)) = (maybe_id, maybe_as) {
                self.frame_mut()?.ids_assigned_in_frame.insert(id);
            }
        }

        self.push_node_with_start_span(Node::Import { from, items }, start_span)
    }

    fn consume_from_path(&mut self, context: &ExpressionContext) -> Result<AstVec<AstIndex>> {
        let mut path = AstVec::new();

        while let Some(item_root) = self.parse_id_or_string(context)? {
            path.push(item_root);

            while self.peek_token() == Some(Token::Dot) {
                self.consume_token();

                let Some(next) = self.parse_id_or_string(&ExpressionContext::restricted())? else {
                    return self.consume_token_and_error(SyntaxError::ExpectedImportModuleId);
                };

                path.push(next);
            }
        }

        if path.is_empty() {
            self.error(SyntaxError::ExpectedPathAfterFrom)
        } else {
            Ok(path)
        }
    }

    // Helper for parse_import(), parses a series of import items
    // e.g.
    //   from baz.qux import foo, 'bar', 'x'
    //   #    ^ You are here, with nested items allowed
    //   #                   ^ Or here, with nested items disallowed
    fn consume_import_items(&mut self, context: &ExpressionContext) -> Result<Vec<ImportItem>> {
        let mut items = Vec::new();
        let mut context = *context;

        loop {
            let Some(item) = self.parse_id_or_string(&context)? else {
                break;
            };
            let name = match self.peek_token_with_context(&context) {
                Some(peeked) if peeked.token == Token::As => {
                    self.consume_token_with_context(&context);
                    match self.parse_id(&context)? {
                        Some((id, _)) => Some(self.push_node(Node::Id(id, None))?),
                        None => return self.error(SyntaxError::ExpectedIdAfterAs),
                    }
                }
                _ => None,
            };

            items.push(ImportItem { item, name });

            match self.peek_token_with_context(&context) {
                Some(peeked) if peeked.token == Token::Comma => {
                    if let Some((_, new_context)) = self.consume_token_with_context(&context) {
                        context = new_context.with_expected_indentation(
                            Indentation::GreaterOrEqual(self.current_indent()),
                        );
                    }
                }
                Some(peeked) if peeked.token == Token::Dot => {
                    return self.consume_token_and_error(SyntaxError::UnexpectedDotAfterImportItem);
                }
                _ => break,
            }
        }

        if items.is_empty() {
            self.error(SyntaxError::ExpectedIdInImportExpression)
        } else {
            Ok(items)
        }
    }

    fn consume_try_expression(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        let outer_context = match self.consume_token_with_context(context) {
            Some((Token::Try, outer_context)) => {
                outer_context.with_expected_indentation(Indentation::Equal(self.current_indent()))
            }
            _ => return self.error(InternalError::UnexpectedToken),
        };

        let start_span = self.current_span();

        let Some(try_block) = self.parse_indented_block()? else {
            return self.consume_token_on_same_line_and_error(ExpectedIndentation::TryBody);
        };

        let mut catch_blocks = AstVec::new();

        while let Some(catch_block) = self.parse_catch_block(&outer_context)? {
            catch_blocks.push(catch_block);
        }

        if catch_blocks.is_empty() {
            return self.error(SyntaxError::ExpectedCatch);
        }

        let finally_block = match self.peek_token_with_context(&outer_context) {
            Some(peeked) if peeked.token == Token::Finally => {
                self.consume_token_with_context(&outer_context);
                if let Some(finally_block) = self.parse_indented_block()? {
                    Some(finally_block)
                } else {
                    return self
                        .consume_token_on_same_line_and_error(ExpectedIndentation::FinallyBody);
                }
            }
            _ => None,
        };

        self.push_node_with_start_span(
            Node::Try(AstTry {
                try_block,
                catch_blocks,
                finally_block,
            }),
            start_span,
        )
    }

    fn parse_catch_block(&mut self, context: &ExpressionContext) -> Result<Option<AstCatch>> {
        if !matches!(
            self.peek_token_with_context(context),
            Some(PeekInfo {
                token: Token::Catch,
                ..
            })
        ) {
            return Ok(None);
        }

        self.consume_token_with_context(context);

        let catch_arg = match self.parse_id_or_wildcard(&ExpressionContext::restricted())? {
            Some(IdOrWildcard::Id(id)) => {
                let type_hint_index = self.parse_type_hint(context)?;
                self.frame_mut()?.ids_assigned_in_frame.insert(id);
                self.push_node(Node::Id(id, type_hint_index))?
            }
            Some(IdOrWildcard::Wildcard(maybe_id)) => {
                let type_hint_index = self.parse_type_hint(context)?;
                self.push_node(Node::Wildcard(maybe_id, type_hint_index))?
            }
            None => return self.consume_token_and_error(SyntaxError::ExpectedCatchArgument),
        };

        let Some(catch_block) = self.parse_indented_block()? else {
            return self.consume_token_on_same_line_and_error(ExpectedIndentation::CatchBody);
        };

        let result = AstCatch {
            arg: catch_arg,
            block: catch_block,
        };

        Ok(Some(result))
    }

    fn consume_let_expression(&mut self, context: &ExpressionContext) -> Result<AstIndex> {
        self.consume_token_with_context(context); // Token::Let

        let mut targets = vec![];

        while let Some(id_or_wildcard) =
            self.parse_id_or_wildcard(&ExpressionContext::permissive())?
        {
            let target_span = self.current_span();
            let target_node = match id_or_wildcard {
                IdOrWildcard::Id(constant_index) => {
                    let type_hint_index = self.parse_type_hint(context)?;
                    Node::Id(constant_index, type_hint_index)
                }
                IdOrWildcard::Wildcard(maybe_id) => {
                    let type_hint_index = self.parse_type_hint(context)?;
                    Node::Wildcard(maybe_id, type_hint_index)
                }
            };
            targets.push(self.push_node_with_span(target_node, target_span)?);

            if let Some(Token::Comma) = self
                .peek_token_with_context(context)
                .map(|token| token.token)
            {
                self.consume_token_with_context(context);
            }
        }

        let Some(last_target) = targets.pop() else {
            return self.error(SyntaxError::ExpectedAssignmentTarget);
        };

        match self.parse_assign_expression(last_target, &targets, context)? {
            Some(val) => Ok(val),
            None => self.consume_token_and_error(SyntaxError::ExpectedAssignmentTarget),
        }
    }

    fn parse_string(&mut self, context: &ExpressionContext) -> Result<Option<ParseStringOutput>> {
        use SyntaxError::*;
        use Token::*;

        let quote = match self.peek_token_with_context(context) {
            Some(PeekInfo {
                token: StringStart(StringType::Normal(quote)),
                ..
            }) => quote,
            Some(PeekInfo {
                token: StringStart(StringType::Raw { .. }),
                ..
            }) => return self.consume_raw_string(context),
            _ => return Ok(None),
        };

        let (_, string_context) = self.consume_token_with_context(context).unwrap();
        let start_span = self.current_span();
        let mut nodes = Vec::new();

        while let Some(next_token) = self.consume_token() {
            match next_token {
                StringLiteral => {
                    let string_literal = self.current_token.slice(self.source);

                    let mut contents = String::with_capacity(string_literal.len());
                    let mut chars = string_literal.chars().peekable();

                    while let Some(c) = chars.next() {
                        if c == '\\' {
                            if let Some(escaped) = self.escape_string_character(&mut chars)? {
                                contents.push(escaped);
                            }
                        } else {
                            contents.push(c);
                        }
                    }

                    nodes.push(StringNode::Literal(self.add_string_constant(&contents)?));
                }
                CurlyOpen => {
                    let Some(expression) =
                        self.parse_expressions(&ExpressionContext::inline(), TempResult::No)?
                    else {
                        return self.consume_token_and_error(ExpectedExpression);
                    };

                    let format = if self.peek_token() == Some(Colon) {
                        self.consume_token(); // :
                        self.consume_format_options()?
                    } else {
                        StringFormatOptions::default()
                    };

                    if self.consume_token() != Some(CurlyClose) {
                        return self.error(ExpectedStringPlaceholderEnd);
                    }

                    nodes.push(StringNode::Expression { expression, format });
                }
                StringEnd => {
                    let contents = match nodes.as_slice() {
                        [] => StringContents::Literal(self.add_string_constant("")?),
                        [StringNode::Literal(literal)] => StringContents::Literal(*literal),
                        _ => StringContents::Interpolated(nodes),
                    };

                    return Ok(Some(ParseStringOutput {
                        string: AstString { quote, contents },
                        span: self.span_with_start(start_span),
                        context: string_context,
                    }));
                }
                _ => return self.error(UnexpectedToken),
            }
        }

        self.error(UnterminatedString)
    }

    fn consume_format_options(&mut self) -> Result<StringFormatOptions> {
        use SyntaxError::*;

        if self.consume_token() == Some(Token::StringLiteral) {
            StringFormatOptions::parse(self.current_token.slice(self.source), &mut self.constants)
                .map_err(|e| self.make_error(FormatStringError(e)))
        } else {
            self.error(ExpectedFormatString)
        }
    }

    fn escape_string_character(&mut self, chars: &mut Peekable<Chars>) -> Result<Option<char>> {
        use SyntaxError::*;

        let Some(next) = chars.next() else {
            return self.error(UnexpectedEscapeInString);
        };

        let result = match next {
            '\\' | '\'' | '"' | '{' => Ok(next),
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            '\r' | '\n' => {
                if next == '\r' {
                    // Skip \n if it follows \r
                    if let Some(&'\n') = chars.peek() {
                        chars.next();
                    } else {
                        return Ok(None);
                    }
                }

                // Skip any whitespace at the start of the line
                while let Some(c) = chars.peek() {
                    if c.is_whitespace() && *c != '\n' {
                        chars.next();
                    } else {
                        break;
                    }
                }

                return Ok(None);
            }
            'x' => match chars.next() {
                Some(c1) if c1.is_ascii_hexdigit() => match chars.next() {
                    Some(c2) if c2.is_ascii_hexdigit() => {
                        // is_ascii_hexdigit already checked
                        let d1 = c1.to_digit(16).unwrap();
                        let d2 = c2.to_digit(16).unwrap();
                        let d = d1 * 16 + d2;
                        if d <= 0x7f {
                            Ok(char::from_u32(d).unwrap())
                        } else {
                            self.error(AsciiEscapeCodeOutOfRange)
                        }
                    }
                    Some(_) => self.error(UnexpectedCharInNumericEscapeCode),
                    None => self.error(UnterminatedNumericEscapeCode),
                },
                Some(_) => self.error(UnexpectedCharInNumericEscapeCode),
                None => self.error(UnterminatedNumericEscapeCode),
            },
            'u' => match chars.next() {
                Some('{') => {
                    let mut code = 0;

                    while let Some(c) = chars.peek().cloned() {
                        if c.is_ascii_hexdigit() {
                            chars.next();
                            code *= 16;
                            code += c.to_digit(16).unwrap();
                        } else {
                            break;
                        }
                    }

                    match chars.next() {
                        Some('}') => match char::from_u32(code) {
                            Some(c) => Ok(c),
                            None => self.error(UnicodeEscapeCodeOutOfRange),
                        },
                        Some(_) => self.error(UnexpectedCharInNumericEscapeCode),
                        None => self.error(UnterminatedNumericEscapeCode),
                    }
                }
                Some(_) => self.error(UnexpectedCharInNumericEscapeCode),
                None => self.error(UnterminatedNumericEscapeCode),
            },
            _ => self.error(UnexpectedEscapeInString),
        };

        result.map(Some)
    }

    fn consume_raw_string(
        &mut self,
        context: &ExpressionContext,
    ) -> Result<Option<ParseStringOutput>> {
        let (delimiter, string_context) = match self.consume_token_with_context(context) {
            Some((Token::StringStart(StringType::Raw(delimiter)), string_context)) => {
                (delimiter, string_context)
            }
            _ => return self.error(InternalError::RawStringParseFailure),
        };

        let start_span = self.current_span();

        let contents = match self.consume_token() {
            Some(Token::StringLiteral) => {
                let contents = self.add_current_slice_as_string_constant()?;
                match self.consume_token() {
                    Some(Token::StringEnd) => contents,
                    _ => return self.error(SyntaxError::UnterminatedString),
                }
            }
            Some(Token::StringEnd) => self.add_string_constant("")?,
            _ => return self.error(SyntaxError::UnterminatedString),
        };

        Ok(Some(ParseStringOutput {
            string: AstString {
                quote: delimiter.quote,
                contents: StringContents::Raw {
                    constant: contents,
                    hash_count: delimiter.hash_count,
                },
            },
            span: self.span_with_start(start_span),
            context: string_context,
        }))
    }

    //// Error helpers

    // Consume an expected token, otherwise error
    fn expect_and_consume_token(
        &mut self,
        expected_token: Token,
        error: ErrorKind,
        context: &ExpressionContext,
    ) -> Result<()> {
        match self.peek_token_with_context(context) {
            Some(_) => match self.consume_token_with_context(context) {
                Some((token, _)) if token == expected_token => Ok(()),
                _ => self.error(error),
            },
            None => self.error(error),
        }
    }

    fn error<E, T>(&mut self, error_type: E) -> Result<T>
    where
        E: Into<ErrorKind>,
    {
        Err(self.make_error(error_type))
    }

    fn make_error<E>(&mut self, error_type: E) -> Error
    where
        E: Into<ErrorKind>,
    {
        #[allow(clippy::let_and_return)]
        let error = Error::new(error_type.into(), self.current_span());

        #[cfg(feature = "panic_on_parser_error")]
        panic!("{error}");

        error
    }

    fn consume_token_on_same_line_and_error<E, T>(&mut self, error_type: E) -> Result<T>
    where
        E: Into<ErrorKind>,
    {
        self.consume_next_token_on_same_line();
        self.error(error_type)
    }

    fn consume_token_and_error<E, T>(&mut self, error_type: E) -> Result<T>
    where
        E: Into<ErrorKind>,
    {
        self.consume_token_with_context(&ExpressionContext::permissive());
        self.error(error_type)
    }

    //// Lexer getters

    fn consume_token(&mut self) -> Option<Token> {
        if let Some(next) = self.lexer.next() {
            self.current_token = next;

            if self.current_token.token == Token::NewLine {
                self.current_line += 1;
            }

            Some(self.current_token.token)
        } else {
            None
        }
    }

    fn peek_token(&mut self) -> Option<Token> {
        self.peek_token_n(0)
    }

    fn peek_token_n(&mut self, n: usize) -> Option<Token> {
        self.lexer.peek(n).map(|peeked| peeked.token)
    }

    fn current_indent(&self) -> usize {
        self.current_token.indent
    }

    fn current_span(&self) -> Span {
        self.current_token.span
    }

    //// Node push helpers

    fn push_node(&mut self, node: Node) -> Result<AstIndex> {
        self.push_node_with_span(node, self.current_span())
    }

    fn push_node_with_span(&mut self, node: Node, span: Span) -> Result<AstIndex> {
        self.ast.push(node, span)
    }

    fn push_node_with_start_span(&mut self, node: Node, start_span: Span) -> Result<AstIndex> {
        self.push_node_with_span(node, self.span_with_start(start_span))
    }

    fn span_with_start(&self, start_span: Span) -> Span {
        Span {
            start: start_span.start,
            end: self.current_span().end,
        }
    }

    fn add_current_slice_as_string_constant(&mut self) -> Result<ConstantIndex> {
        self.add_string_constant(self.current_token.slice(self.source))
    }

    fn add_string_constant(&mut self, s: &str) -> Result<ConstantIndex> {
        match self.constants.add_string(s) {
            Ok(result) => Ok(result),
            Err(_) => self.error(InternalError::ConstantPoolCapacityOverflow),
        }
    }

    // Peeks past whitespace, comments, and newlines until the next token is found
    //
    // Tokens on following lines will only be returned if the expression context allows linebreaks.
    //
    // If expected indentation is specified in the expression context, then the next token
    // needs to have matching indentation, otherwise `None` is returned.
    fn peek_token_with_context(&mut self, context: &ExpressionContext) -> Option<PeekInfo> {
        use Token::*;

        let mut peek_count = 0;
        let mut same_line = true;
        let start_indent = self.current_indent();

        while let Some(peeked) = self.lexer.peek(peek_count) {
            match peeked.token {
                NewLine => same_line = false,
                Whitespace | CommentMulti | CommentSingle => {}
                token => {
                    let result = Some(PeekInfo {
                        token,
                        peek_count,
                        info: peeked.clone(),
                    });

                    let result = if same_line {
                        result
                    } else if context.allow_linebreaks {
                        use Indentation::*;
                        match context.expected_indentation {
                            GreaterThan(expected_indent) if peeked.indent > expected_indent => {
                                result
                            }
                            GreaterOrEqual(expected_indent) if peeked.indent >= expected_indent => {
                                result
                            }
                            Equal(expected_indent) if peeked.indent == expected_indent => result,
                            Greater if peeked.indent > start_indent => result,
                            Flexible => result,
                            _ => None,
                        }
                    } else {
                        None
                    };

                    return result;
                }
            }

            peek_count += 1;
        }

        None
    }

    // Consumes the next token depending on the rules of the current expression context
    //
    // It's expected that a peek has been performed (see peek_token_with_context) to check that the
    // current expression context allows for the token to be consumed.
    //
    // If the expression context allows linebreaks and its expected indentation is set to Greater,
    // and indentation is found, then a new context will be returned with
    // a) the new expected indentation, and b) the `allow_map_block` flag set to true.
    //
    // See also: `consume_until_token_with_context()`.
    fn consume_token_with_context(
        &mut self,
        context: &ExpressionContext,
    ) -> Option<(Token, ExpressionContext)> {
        let start_line = self.current_line;
        let start_indent = self.current_indent();

        while let Some(token) = self.consume_token() {
            if !(token.is_whitespace_including_newline()) {
                let is_indented_block = self.current_line > start_line
                    && self.current_indent() > start_indent
                    && context.allow_linebreaks
                    && matches!(context.expected_indentation, Indentation::Greater);

                let new_context = if is_indented_block {
                    ExpressionContext {
                        expected_indentation: Indentation::Equal(self.current_indent()),
                        allow_map_block: true,
                        ..*context
                    }
                } else {
                    *context
                };

                return Some((token, new_context));
            }
        }

        None
    }

    // Consumes whitespace, comments, and newlines up until the next token
    //
    // See the description of `consume_token_with_context()` for more information.
    fn consume_until_token_with_context(
        &mut self,
        context: &ExpressionContext,
    ) -> Option<ExpressionContext> {
        let start_line = self.current_line;
        let start_indent = self.current_indent();

        while let Some(peeked) = self.lexer.peek(0) {
            if peeked.token.is_whitespace_including_newline() {
                self.consume_token();
            } else {
                let is_indented_block = peeked.span.start.line > start_line
                    && peeked.indent > start_indent
                    && context.allow_linebreaks
                    && matches!(context.expected_indentation, Indentation::Greater);

                let new_context = if is_indented_block {
                    ExpressionContext {
                        expected_indentation: Indentation::Equal(peeked.indent),
                        allow_map_block: true,
                        ..*context
                    }
                } else {
                    *context
                };

                return Some(new_context);
            }
        }

        None
    }

    // Peeks past whitespace on the same line until the next token is found
    fn peek_next_token_on_same_line(&mut self) -> Option<Token> {
        let mut peek_count = 0;

        while let Some(peeked) = self.peek_token_n(peek_count) {
            match peeked {
                token if token.is_whitespace() => {}
                token => return Some(token),
            }

            peek_count += 1;
        }

        None
    }

    // Consumes whitespace on the same line up until the next token
    fn consume_until_next_token_on_same_line(&mut self) {
        while let Some(peeked) = self.peek_token() {
            match peeked {
                token if token.is_whitespace() => {}
                _ => return,
            }

            self.consume_token();
        }
    }

    // Consumes whitespace on the same line and returns the next token
    fn consume_next_token_on_same_line(&mut self) -> Option<Token> {
        while let Some(peeked) = self.peek_token() {
            match peeked {
                token if token.is_whitespace() => {}
                _ => return self.consume_token(),
            }

            self.consume_token();
        }

        None
    }

    fn frame(&self) -> Result<&Frame> {
        match self.frame_stack.last() {
            Some(frame) => Ok(frame),
            None => Err(Error::new(
                InternalError::MissingFrame.into(),
                Span::default(),
            )),
        }
    }

    fn frame_mut(&mut self) -> Result<&mut Frame> {
        match self.frame_stack.last_mut() {
            Some(frame) => Ok(frame),
            None => Err(Error::new(
                InternalError::MissingFrame.into(),
                Span::default(),
            )),
        }
    }
}

// Used by Parser::parse_expressions() to determine if comma-separated values should be stored in a
// Tuple or a TempTuple.
enum TempResult {
    No,
    Yes,
}

// The first operator that's above the pipe operator -> in precedence.
// Q: Why is this needed?
// A: Function calls without parentheses aren't currently treated as operators (a Call operator
//    with higher precedence than Pipe would allow this to go away, but would likely take quite a
//    bit of reworking. All calls to parse_call_args will need to reworked).
//    parse_call_args needs to parse arguments as expressions with a minimum precedence that
//    excludes piping, otherwise `f g >> x` would be parsed as `f (g >> x)` instead of `(f g) >> x`.
const MIN_PRECEDENCE_AFTER_PIPE: u8 = 3;

fn operator_precedence(op: Token) -> Option<(u8, u8)> {
    use Token::*;
    let priority = match op {
        // Pipe operator, left-associative
        Arrow => (1, 2),
        // Compound assignments, right-associative
        AddAssign | SubtractAssign | MultiplyAssign | DivideAssign | RemainderAssign
        | PowerAssign => (4, MIN_PRECEDENCE_AFTER_PIPE),
        // Logical operators, left-associative
        Or => (7, 8),
        And => (9, 10),
        // Comparisons, right-associative
        Equal | NotEqual => (12, 11),
        Greater | GreaterOrEqual | Less | LessOrEqual => (14, 13),
        // Arithmetic operators, left-associative
        Add | Subtract => (15, 16),
        Multiply | Divide | Remainder => (17, 18),
        Power => (19, 20),
        _ => return None,
    };
    Some(priority)
}

// Returned by Parser::peek_token_with_context()
#[derive(Debug)]
struct PeekInfo {
    token: Token,
    peek_count: usize,
    info: LexedToken,
}

// Returned by Parser::parse_id_or_wildcard()
#[derive(Debug)]
enum IdOrWildcard {
    Id(ConstantIndex),
    Wildcard(Option<ConstantIndex>),
}

// Returned by Parser::parse_string()
struct ParseStringOutput {
    string: AstString,
    span: Span,
    context: ExpressionContext,
}
