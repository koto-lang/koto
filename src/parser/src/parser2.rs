use {
    crate::{error::*, *},
    koto_lexer::{make_end_position, make_span, make_start_position, Lexer, Span, Token},
    std::{collections::HashSet, str::FromStr},
};

macro_rules! internal_error {
    ($error:ident, $parser:expr) => {{
        let extras = &$parser.lexer.extras;
        let error = ParserError::new(
            InternalError::$error.into(),
            make_span(
                $parser.lexer.source(),
                extras.line_number,
                extras.line_start,
                &$parser.lexer.span(),
            ),
        );
        #[cfg(panic_on_parser_error)]
        {
            panic!(error);
        }
        Err(error)
    }};
}

macro_rules! syntax_error {
    ($error:ident, $parser:expr) => {{
        let extras = &$parser.lexer.extras;
        let error = ParserError::new(
            SyntaxError::$error.into(),
            make_span(
                $parser.lexer.source(),
                extras.line_number,
                extras.line_start,
                &$parser.lexer.span(),
            ),
        );
        #[cfg(panic_on_parser_error)]
        {
            panic!(error);
        }
        Err(error)
    }};
}

fn trim_str(s: &str, trim_from_start: usize, trim_from_end: usize) -> &str {
    let start = trim_from_start;
    let end = s.len() - trim_from_end;
    &s[start..end]
}

#[derive(Default)]
struct Frame {
    ids_assigned_in_scope: HashSet<ConstantIndex>,
    captures: HashSet<ConstantIndex>,
    _top_level: bool,
}

impl Frame {
    fn local_count(&self) -> usize {
        self.ids_assigned_in_scope
            .difference(&self.captures)
            .count()
    }
}

pub struct Parser<'source, 'constants> {
    ast: Ast,
    lexer: Lexer<'source>,
    constants: &'constants mut ConstantPool,
    frame_stack: Vec<Frame>,
}

impl<'source, 'constants> Parser<'source, 'constants> {
    pub fn parse(
        source: &'source str,
        constants: &'constants mut ConstantPool,
    ) -> Result<Ast, ParserError> {
        let capacity_guess = source.len() / 4;
        let mut parser = Parser {
            ast: Ast::with_capacity(capacity_guess),
            lexer: Lexer::new(source),
            constants,
            frame_stack: Vec::new(),
        };

        let main_block = parser.parse_main_block()?;
        parser.ast.set_entry_point(main_block);

        Ok(parser.ast)
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
        self.frame_stack.push(Frame {
            _top_level: true,
            ..Frame::default()
        });

        let mut body = Vec::new();
        while self.peek_token().is_some() {
            if let Some(expression) = self.parse_new_line()? {
                body.push(expression);
            }

            match self.consume_token() {
                Some(token) => match token {
                    Token::NewLine => continue,
                    unexpected => {
                        unimplemented!("parse_main_block: Unimplemented token: {:?}", unexpected)
                    }
                },
                None => continue,
            }
        }

        let result = self.ast.push(
            Node::MainBlock {
                body,
                local_count: self.frame()?.local_count(),
            },
            Span::default(),
        )?;

        self.frame_stack.pop();
        Ok(result)
    }

    fn parse_function(
        &mut self,
        primary_expression: bool,
    ) -> Result<Option<AstIndex>, ParserError> {
        if let Some(Token::Function) = self.peek_token() {
            self.consume_token();

            let span_start = make_start_position(
                self.lexer.source(),
                self.lexer.extras.line_number,
                self.lexer.extras.line_start,
                &self.lexer.span(),
            );

            // args
            let mut args = Vec::new();
            while let Some(constant_index) = self.parse_id() {
                args.push(constant_index);
            }

            if self.skip_whitespace_and_next() != Some(Token::Function) {
                return syntax_error!(ExpectedFunctionArgsEnd, self);
            }

            // body
            let mut function_frame = Frame::default();
            function_frame.ids_assigned_in_scope.extend(args.clone());
            self.frame_stack.push(function_frame);

            let current_indent = self.lexer.extras.indent;

            let mut body = Vec::new();
            let expected_indent = match self.peek_token() {
                Some(Token::NewLineIndented)
                    if primary_expression && self.lexer.extras.indent > current_indent =>
                {
                    self.consume_token();
                    Some(self.lexer.extras.indent)
                }
                _ => None,
            };

            while self.peek_token().is_some() {
                if let Some(expression) = self.parse_new_line()? {
                    body.push(expression);
                }

                if let Some(expected_indent) = expected_indent {
                    match self.peek_token() {
                        Some(token) => match token {
                            Token::NewLineIndented
                                if self.lexer.extras.indent == expected_indent =>
                            {
                                self.consume_token();
                                continue;
                            }
                            Token::NewLineIndented
                                if self.lexer.extras.indent < expected_indent =>
                            {
                                break
                            }
                            Token::NewLineIndented
                                if self.lexer.extras.indent > expected_indent =>
                            {
                                return syntax_error!(UnexpectedIndentation, self);
                            }
                            Token::NewLine => break,
                            unexpected => unimplemented!(
                                "parse_function: Unimplemented token: {:?}",
                                unexpected
                            ),
                        },
                        None => continue,
                    }
                } else {
                    break;
                }
            }

            let span_end = make_end_position(
                self.lexer.source(),
                self.lexer.extras.line_number,
                self.lexer.extras.line_start,
                &self.lexer.span(),
            );

            let result = self.ast.push(
                Node::Function(Function {
                    args,
                    captures: vec![], // TODO
                    local_count: self.frame()?.local_count(),
                    body,
                    is_instance_function: false, // TODO
                }),
                Span {
                    start: span_start,
                    end: span_end,
                },
            )?;

            self.frame_stack.pop();
            Ok(Some(result))
        } else {
            internal_error!(FunctionParseFailure, self)
        }
    }

    fn parse_new_line(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if let for_loop @ Some(_) = self.parse_for_loop(None)? {
            return Ok(for_loop);
        } else {
            self.parse_primary_expressions()
        }
    }

    fn parse_primary_expressions(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if let Some(first) = self.parse_primary_expression()? {
            let mut expressions = vec![first];
            while let Some(Token::Separator) = self.skip_whitespace_and_peek() {
                self.consume_token();
                if let Some(next_expression) = self.parse_primary_expression()? {
                    expressions.push(next_expression);
                } else {
                    return syntax_error!(ExpectedExpression, self);
                }
            }
            if expressions.len() == 1 {
                Ok(Some(first))
            } else {
                Ok(Some(self.push_node(Node::Expressions(expressions))?))
            }
        } else {
            Ok(None)
        }
    }

    fn parse_primary_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression(0)
    }

    fn parse_non_primary_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        self.parse_expression(1)
    }

    fn parse_expression(&mut self, min_precedence: u8) -> Result<Option<AstIndex>, ParserError> {
        use Token::*;

        let primary_expression = min_precedence == 0;

        let mut lhs = {
            // ID expressions are broken out to allow function calls in first position
            if let Some(id_expression) = self.parse_id_expression(primary_expression)? {
                id_expression
            } else if let Some(term) = self.parse_term(primary_expression)? {
                match self.peek_token() {
                    range @ Some(Token::Range) | range @ Some(Token::RangeInclusive) => {
                        let inclusive = range == Some(Token::RangeInclusive);
                        self.consume_token();
                        if let Some(rhs) = self.parse_term(false)? {
                            return Ok(Some(self.push_node(Node::Range {
                                start: term,
                                end: rhs,
                                inclusive,
                            })?));
                        } else {
                            return syntax_error!(ExpectedRangeRhs, self);
                        }
                    }
                    _ => term,
                }
            } else {
                return Ok(None);
            }
        };

        while let Some(next) = self.skip_whitespace_and_peek() {
            match next {
                NewLine | NewLineIndented => break,
                For => {
                    return self.parse_for_loop(Some(lhs));
                }
                Assign => match self.ast.node(lhs).node {
                    Node::Id(id_index) => {
                        self.consume_token();

                        if let Some(rhs) = self.parse_primary_expressions()? {
                            let node = Node::Assign {
                                target: AssignTarget::Id {
                                    id_index: lhs,
                                    scope: Scope::Local, // TODO
                                },
                                expression: rhs,
                            };
                            self.frame_mut()?.ids_assigned_in_scope.insert(id_index);
                            lhs = self.push_node(node)?;
                        } else {
                            return syntax_error!(ExpectedRhsExpression, self);
                        }
                    }
                    _ => {
                        return syntax_error!(ExpectedAssignmentTarget, self);
                    }
                },
                AssignAdd | AssignSubtract | AssignMultiply | AssignDivide | AssignModulo => {
                    unimplemented!("Unimplemented assignment operator")
                }
                _ => {
                    if let Some(priority) = operator_precedence(next) {
                        if priority < min_precedence {
                            break;
                        }

                        let op = self.consume_token().unwrap();

                        if let Some(rhs) = self.parse_expression(priority)? {
                            lhs = self.push_ast_op(op, lhs, rhs)?;
                        } else {
                            return syntax_error!(ExpectedRhsExpression, self);
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        Ok(Some(lhs))
    }

    fn parse_id(&mut self) -> Option<ConstantIndex> {
        if let Some(Token::Id) = self.skip_whitespace_and_peek() {
            self.consume_token();
            Some(self.constants.add_string(self.lexer.slice()) as u32)
        } else {
            None
        }
    }

    fn parse_id_expression(
        &mut self,
        primary_expression: bool,
    ) -> Result<Option<AstIndex>, ParserError> {
        if let Some(constant_index) = self.parse_id() {
            let id_index = self.push_node(Node::Id(constant_index))?;

            let result = match self.peek_token() {
                Some(Token::Whitespace) if primary_expression => {
                    self.consume_token();
                    if let Some(expression) = self.parse_expression(1)? {
                        let mut args = vec![expression];

                        while let Some(expression) = self.parse_expression(1)? {
                            args.push(expression);
                        }

                        self.push_node(Node::Call {
                            function: id_index,
                            args,
                        })?
                    } else {
                        id_index
                    }
                }
                Some(Token::ParenOpen) => {
                    self.consume_token();

                    let mut args = Vec::new();

                    while let Some(expression) = self.parse_primary_expression()? {
                        args.push(expression);
                    }

                    if let Some(Token::ParenClose) = self.peek_token() {
                        self.consume_token();
                        self.push_node(Node::Call {
                            function: id_index,
                            args,
                        })?
                    } else {
                        id_index
                    }
                }
                _ => id_index,
            };

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn parse_term(&mut self, primary_expression: bool) -> Result<Option<AstIndex>, ParserError> {
        use Node::*;

        if let Some(token) = self.skip_whitespace_and_peek() {
            let result = match token {
                Token::True => self.consume_token_and_push_node(BoolTrue)?,
                Token::False => self.consume_token_and_push_node(BoolFalse)?,
                Token::ParenOpen => {
                    self.consume_token();

                    let expression = if let Some(expression) = self.parse_primary_expression()? {
                        expression
                    } else {
                        self.push_node(Empty)?
                    };

                    if let Some(Token::ParenClose) = self.peek_token() {
                        self.consume_token();
                        expression
                    } else {
                        return syntax_error!(ExpectedCloseParen, self);
                    }
                }
                Token::Number => match f64::from_str(self.lexer.slice()) {
                    Ok(n) => {
                        if n == 0.0 {
                            self.consume_token_and_push_node(Number0)?
                        } else if n == 1.0 {
                            self.consume_token_and_push_node(Number1)?
                        } else {
                            let constant_index = self.constants.add_f64(n) as u32;
                            self.consume_token_and_push_node(Number(constant_index))?
                        }
                    }
                    Err(_) => {
                        return internal_error!(NumberParseFailure, self);
                    }
                },
                Token::Str => {
                    let s = trim_str(self.lexer.slice(), 1, 1);
                    let constant_index = self.constants.add_string(s) as u32;
                    self.consume_token_and_push_node(Str(constant_index))?
                }
                Token::Id => {
                    let constant_index = self.constants.add_string(self.lexer.slice()) as u32;
                    self.consume_token_and_push_node(Id(constant_index))?
                }
                Token::ListStart => {
                    self.consume_token();
                    let mut entries = Vec::new();
                    while let Some(entry) = self.parse_term(false)? {
                        entries.push(entry);
                    }
                    if self.skip_whitespace_and_next() != Some(Token::ListEnd) {
                        return syntax_error!(ExpectedListEnd, self);
                    }
                    self.push_node(List(entries))?
                }
                Token::MapStart => {
                    self.consume_token();
                    let mut entries = Vec::new();

                    loop {
                        if let Some(key) = self.parse_id() {
                            if self.skip_whitespace_and_next() != Some(Token::Colon) {
                                return syntax_error!(ExpectedMapSeparator, self);
                            }

                            if let Some(value) = self.parse_primary_expression()? {
                                entries.push((key, value));
                            } else {
                                return syntax_error!(ExpectedMapValue, self);
                            }

                            if self.skip_whitespace_and_peek() == Some(Token::Separator) {
                                self.consume_token();
                                continue;
                            } else {
                                break;
                            }
                        }
                    }

                    if self.skip_whitespace_and_next() != Some(Token::MapEnd) {
                        return syntax_error!(ExpectedMapEnd, self);
                    }

                    self.push_node(Map(entries))?
                }
                Token::If => return self.parse_if_expression(),
                Token::Function => return self.parse_function(primary_expression),
                _ => return Ok(None),
            };

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn parse_for_loop(&mut self, body: Option<AstIndex>) -> Result<Option<AstIndex>, ParserError> {
        if self.skip_whitespace_and_peek() != Some(Token::For) {
            return Ok(None);
        }

        let current_indent = self.lexer.extras.indent;

        self.consume_token();

        let mut args = Vec::new();
        while let Some(constant_index) = self.parse_id() {
            args.push(constant_index);
            self.frame_mut()?
                .ids_assigned_in_scope
                .insert(constant_index);
            if self.skip_whitespace_and_peek() == Some(Token::Separator) {
                self.consume_token();
            }
        }
        if args.is_empty() {
            return syntax_error!(ExpectedForArgs, self);
        }

        if self.skip_whitespace_and_next() != Some(Token::In) {
            return syntax_error!(ExpectedForInKeyword, self);
        }

        let mut ranges = Vec::new();
        while let Some(range) = self.parse_non_primary_expression()? {
            ranges.push(range);

            if self.skip_whitespace_and_peek() != Some(Token::Separator) {
                break;
            }

            self.consume_token();
        }
        if ranges.is_empty() {
            return syntax_error!(ExpectedForRanges, self);
        }

        let condition = if self.skip_whitespace_and_peek() == Some(Token::If) {
            self.consume_token();
            if let Some(condition) = self.parse_primary_expression()? {
                Some(condition)
            } else {
                return syntax_error!(ExpectedForCondition, self);
            }
        } else {
            None
        };

        let body = if let Some(body) = body {
            body
        } else if let Some(body) = self.parse_indented_block(current_indent)? {
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

    fn parse_if_expression(&mut self) -> Result<Option<AstIndex>, ParserError> {
        if self.peek_token() != Some(Token::If) {
            return Ok(None);
        }

        let current_indent = self.lexer.extras.indent;

        self.consume_token();
        let condition = match self.parse_primary_expression()? {
            Some(condition) => condition,
            None => return syntax_error!(ExpectedIfCondition, self),
        };

        let result = if self.skip_whitespace_and_peek() == Some(Token::Then) {
            self.consume_token();
            dbg!(self.peek_token());
            let then_node = match self.parse_primary_expression()? {
                Some(then_node) => then_node,
                None => return syntax_error!(ExpectedThenExpression, self),
            };
            dbg!(then_node);
            let else_node = if self.skip_whitespace_and_peek() == Some(Token::Else) {
                self.consume_token();
                dbg!(self.peek_token());
                match self.parse_primary_expression()? {
                    Some(else_node) => Some(else_node),
                    None => return syntax_error!(ExpectedElseExpression, self),
                }
            } else {
                None
            };

            dbg!(self.peek_token());
            self.push_node(Node::If(AstIf {
                condition,
                then_node,
                else_if_blocks: vec![],
                else_node,
            }))?
        } else if let Some(then_node) = self.parse_indented_block(current_indent)? {
            let mut else_if_blocks = Vec::new();

            while self.lexer.extras.indent == current_indent {
                self.consume_token(); // NewLine|Indented
                if let Some(Token::ElseIf) = self.skip_whitespace_and_peek() {
                    self.consume_token();
                    if let Some(else_if_condition) = self.parse_primary_expression()? {
                        if let Some(else_if_block) = self.parse_indented_block(current_indent)? {
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

            let else_node = if self.lexer.extras.indent == current_indent {
                if let Some(Token::Else) = self.skip_whitespace_and_peek() {
                    self.consume_token();
                    if let Some(else_block) = self.parse_indented_block(current_indent)? {
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

    fn parse_indented_block(
        &mut self,
        current_indent: usize,
    ) -> Result<Option<AstIndex>, ParserError> {
        use Token::{NewLine, NewLineIndented};

        if self.skip_whitespace_and_peek() != Some(NewLineIndented) {
            return Ok(None);
        }

        self.consume_token();
        let block_indent = self.lexer.extras.indent;

        if block_indent <= current_indent {
            return Ok(None);
        }

        let mut body = Vec::new();
        while let Some(expression) = self.parse_line()? {
            body.push(expression);

            match self.skip_whitespace_and_peek() {
                Some(token) => {
                    let next_indent = self.lexer.extras.indent;
                    match token {
                        NewLineIndented if next_indent == block_indent => {
                            self.consume_token();
                            continue;
                        }
                        NewLineIndented if next_indent < block_indent => break,
                        NewLineIndented if next_indent > block_indent => {
                            return syntax_error!(UnexpectedIndentation, self);
                        }
                        NewLine => break,
                        unexpected => {
                            unimplemented!("parse_function: Unimplemented token: {:?}", unexpected)
                        }
                    }
                }
                None => continue,
            }
        }

        if body.len() == 1 {
            Ok(Some(*body.first().unwrap()))
        } else {
            Ok(Some(self.ast.push(Node::Block(body), Span::default())?))
        }
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
        self.push_node(Node::Op {
            op: ast_op,
            lhs,
            rhs,
        })
    }

    fn consume_token_and_push_node(&mut self, node: Node) -> Result<AstIndex, ParserError> {
        self.consume_token();
        self.push_node(node)
    }

    fn peek_token(&mut self) -> Option<Token> {
        self.lexer.peek()
    }

    fn consume_token(&mut self) -> Option<Token> {
        self.lexer.next()
    }

    fn push_node(&mut self, node: Node) -> Result<AstIndex, ParserError> {
        self.ast.push(
            node,
            make_span(
                self.lexer.source(),
                self.lexer.extras.line_number,
                self.lexer.extras.line_start,
                &self.lexer.span(),
            ),
        )
    }

    fn skip_whitespace_and_peek(&mut self) -> Option<Token> {
        loop {
            let peeked = self.peek_token();

            match peeked {
                Some(Token::Whitespace) => {}
                Some(token) => return Some(token),
                None => return None,
            }

            self.lexer.next();
            continue;
        }
    }

    fn skip_whitespace_and_next(&mut self) -> Option<Token> {
        loop {
            let peeked = self.peek_token();

            match peeked {
                Some(Token::Whitespace) => {}
                Some(_) => return self.lexer.next(),
                None => return None,
            }

            self.lexer.next();
            continue;
        }
    }
}

fn operator_precedence(op: Token) -> Option<u8> {
    use Token::*;
    let priority = match op {
        Or => 1,
        And => 2,
        Equal | NotEqual => 3,
        Greater | GreaterOrEqual | Less | LessOrEqual => 4,
        Add | Subtract => 5,
        Multiply | Divide | Modulo => 6,
        _ => return None,
    };
    Some(priority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use {crate::constant_pool::Constant, Node::*};

    fn check_ast(source: &str, expected_ast: &[Node], expected_constants: Option<&[Constant]>) {
        println!("{}", source);

        let mut constants = ConstantPool::default();
        match Parser::parse(source, &mut constants) {
            Ok(ast) => {
                for (i, (ast_node, expected_node)) in
                    ast.nodes().iter().zip(expected_ast.iter()).enumerate()
                {
                    assert_eq!(ast_node.node, *expected_node, "Mismatch at position {}", i);
                }
                assert_eq!(ast.nodes().len(), expected_ast.len());

                if let Some(expected_constants) = expected_constants {
                    for (constant, expected_constant) in
                        constants.iter().zip(expected_constants.iter())
                    {
                        assert_eq!(constant, *expected_constant);
                    }
                    assert_eq!(constants.len(), expected_constants.len());
                }
            }
            Err(error) => panic!("{}", error),
        }
    }

    mod values {
        use super::*;

        #[test]
        fn literals() {
            let source = "\
true
false
1
1.5
\"hello\"
a
()";
            check_ast(
                source,
                &[
                    BoolTrue,
                    BoolFalse,
                    Number1,
                    Number(0),
                    Str(1),
                    Id(2),
                    Empty,
                    MainBlock {
                        body: vec![0, 1, 2, 3, 4, 5, 6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Number(1.5),
                    Constant::Str("hello"),
                    Constant::Str("a"),
                ]),
            )
        }

        #[test]
        fn list() {
            let source = "[0 n \"test\" n -1]";
            check_ast(
                source,
                &[
                    Number0,
                    Id(0),
                    Str(1),
                    Id(0),
                    Number(2),
                    List(vec![0, 1, 2, 3, 4]),
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("n"),
                    Constant::Str("test"),
                    Constant::Number(-1.0),
                ]),
            )
        }

        #[test]
        fn map_inline() {
            let source = "{foo: 42, bar: \"hello\"}";
            check_ast(
                source,
                &[
                    Number(1),
                    Str(3),
                    Map(vec![(0, 0), (2, 1)]), // map entries are constant/ast index pairs
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("hello"),
                ]),
            )
        }

        #[test]
        fn ranges() {
            let source = "\
0..1
0..=1
(0 + 1)..(1 + 1)";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    Number0,
                    Number1,
                    Range {
                        start: 3,
                        end: 4,
                        inclusive: true,
                    }, // 5
                    Number0,
                    Number1,
                    Op {
                        op: AstOp::Add,
                        lhs: 6,
                        rhs: 7,
                    },
                    Number1,
                    Number1, // 10
                    Op {
                        op: AstOp::Add,
                        lhs: 9,
                        rhs: 10,
                    },
                    Range {
                        start: 8,
                        end: 11,
                        inclusive: false,
                    },
                    MainBlock {
                        body: vec![2, 5, 12],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn multiple_expressions() {
            let source = "0, 1, 0";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Number0,
                    Expressions(vec![0, 1, 2]),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                None,
            )
        }
    }

    mod assignment {
        use super::*;
        use crate::node::{AssignTarget, Scope};

        #[test]
        fn single() {
            let source = "a = 1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 1,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn multi_2_to_1() {
            let source = "x = 1, 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Number0,
                    Expressions(vec![1, 2]),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn addition_subtraction() {
            let source = "1 - 0 + 1";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    Number1,
                    Op {
                        op: AstOp::Add,
                        lhs: 1,
                        rhs: 2,
                    },
                    Op {
                        op: AstOp::Subtract,
                        lhs: 0,
                        rhs: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn add_multiply() {
            let source = "1 + 0 * 1 + 0";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    Number1,
                    Op {
                        op: AstOp::Multiply,
                        lhs: 1,
                        rhs: 2,
                    },
                    Number0,
                    Op {
                        op: AstOp::Add,
                        lhs: 3,
                        rhs: 4,
                    },
                    Op {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn with_parentheses() {
            let source = "(1 + 0) * (1 + 0)";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    Op {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    Number0,
                    Op {
                        op: AstOp::Add,
                        lhs: 3,
                        rhs: 4,
                    },
                    Op {
                        op: AstOp::Multiply,
                        lhs: 2,
                        rhs: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn logic() {
            let source = "0 < 1 and 1 > 0 or true";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Op {
                        op: AstOp::Less,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    Number0,
                    Op {
                        op: AstOp::Greater,
                        lhs: 3,
                        rhs: 4,
                    },
                    Op {
                        op: AstOp::And,
                        lhs: 2,
                        rhs: 5,
                    },
                    BoolTrue,
                    Op {
                        op: AstOp::Or,
                        lhs: 6,
                        rhs: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn string_and_id() {
            let source = "\"hello\" + x";
            check_ast(
                source,
                &[
                    Str(0),
                    Id(1),
                    Op {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("hello"), Constant::Str("x")]),
            )
        }
    }

    mod control_flow {
        use super::*;

        #[test]
        fn if_inline() {
            let source = "1 + if true then 0 else 1";
            check_ast(
                source,
                &[
                    Number1,
                    BoolTrue,
                    Number0,
                    Number1,
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![],
                        else_node: Some(3),
                    }),
                    Op {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 4,
                    },
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn if_block() {
            let source = "\
a = if false
  0
elseif true
  1
elseif false
  0
else
  1
a";
            check_ast(
                source,
                &[
                    Id(0),
                    BoolFalse,
                    Number0,
                    BoolTrue,
                    Number1,
                    BoolFalse, // 5
                    Number0,
                    Number1,
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![(3, 4), (5, 6)],
                        else_node: Some(7),
                    }),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 8,
                    },
                    Id(0),
                    MainBlock {
                        body: vec![9, 10],
                        local_count: 1,
                    }, // 10
                ],
                None,
            )
        }
    }

    mod loops {
        use super::*;

        #[test]
        fn for_inline() {
            let source = "x for x in 0..1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Number1,
                    Range {
                        start: 1,
                        end: 2,
                        inclusive: false,
                    },
                    For(AstFor {
                        args: vec![0],
                        ranges: vec![3],
                        condition: None,
                        body: 0,
                    }),
                    MainBlock {
                        body: vec![4],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn for_inline_conditional() {
            let source = "x for x in y if x == 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(0),
                    Number0,
                    Op {
                        op: AstOp::Equal,
                        lhs: 2,
                        rhs: 3,
                    },
                    For(AstFor {
                        args: vec![0],
                        ranges: vec![1],
                        condition: Some(4),
                        body: 0,
                    }), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn for_block() {
            let source = "\
for x in y if x > 0
  f x";
            check_ast(
                source,
                &[
                    Id(1),
                    Id(0),
                    Number0,
                    Op {
                        op: AstOp::Greater,
                        lhs: 1,
                        rhs: 2,
                    },
                    Id(2),
                    Id(0), // 5
                    Call {
                        function: 4,
                        args: vec![5],
                    },
                    For(AstFor {
                        args: vec![0],   // constant 0
                        ranges: vec![0], // ast 0
                        condition: Some(3),
                        body: 6,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }
    }

    mod functions {
        use super::*;
        use crate::node::{AssignTarget, Scope};

        #[test]
        fn inline_no_args() {
            let source = "a = || 42";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(1),
                    Function(Function {
                        args: vec![],
                        captures: vec![],
                        local_count: 0,
                        body: vec![1],
                        is_instance_function: false,
                    }),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 2,
                    },
                    MainBlock {
                        body: vec![3],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Number(42.0)]),
            )
        }

        #[test]
        fn inline_two_args() {
            let source = "|x y| x + y";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Op {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Function(Function {
                        args: vec![0, 1],
                        captures: vec![],
                        local_count: 2,
                        body: vec![2],
                        is_instance_function: false,
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn with_body() {
            let source = "\
f = |x|
  x = x + 1
  x
f 42";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(1),
                    Number1,
                    Op {
                        op: AstOp::Add,
                        lhs: 2,
                        rhs: 3,
                    },
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 1,
                            scope: Scope::Local,
                        },
                        expression: 4,
                    }, // 5
                    Id(1),
                    Function(Function {
                        args: vec![1],
                        captures: vec![],
                        local_count: 1,
                        body: vec![5, 6],
                        is_instance_function: false,
                    }),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 7,
                    },
                    Id(0),
                    Number(2), // 10
                    Call {
                        function: 9,
                        args: vec![10],
                    },
                    MainBlock {
                        body: vec![8, 11],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Number(42.0),
                ]),
            )
        }

        #[test]
        fn with_body_nested() {
            let source = "\
f = |x|
  y = |z|
    z
  y x
f 42";
            check_ast(
                source,
                &[
                    Id(0), // f
                    Id(2), // y
                    Id(3), // z
                    Function(Function {
                        args: vec![3],
                        captures: vec![],
                        local_count: 1,
                        body: vec![2],
                        is_instance_function: false,
                    }),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 1,
                            scope: Scope::Local,
                        },
                        expression: 3,
                    },
                    Id(2), // y // 5
                    Id(1), // x
                    Call {
                        function: 5,
                        args: vec![6],
                    },
                    Function(Function {
                        args: vec![1],
                        captures: vec![],
                        local_count: 2,
                        body: vec![4, 7],
                        is_instance_function: false,
                    }),
                    Assign {
                        target: AssignTarget::Id {
                            id_index: 0,
                            scope: Scope::Local,
                        },
                        expression: 8,
                    },
                    Id(0), // f // 10
                    Number(4),
                    Call {
                        function: 10,
                        args: vec![11],
                    },
                    MainBlock {
                        body: vec![9, 12],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Number(42.0),
                ]),
            )
        }
    }
}
