use {koto_lexer::Span, std::fmt};

#[derive(Debug)]
pub enum InternalError {
    AstCapacityOverflow,
    MissingScope,
    NumberParseFailure,
    FunctionParseFailure,
}

#[derive(Debug)]
pub enum SyntaxError {
    UnexpectedToken,
    ExpectedExpression,
    ExpectedRhsExpression,
    ExpectedCloseParen,
    ExpectedListEnd,
    ExpectedMapSeparator,
    ExpectedMapValue,
    ExpectedMapEnd,
    ExpectedIfCondition,
    ExpectedThenKeyword,
    ExpectedThenNode,
    ExpectedElseNode,
    ExpectedAssignmentTarget,
    ExpectedFunctionArgsEnd,
    ExpectedCallArgsEnd,
    UnexpectedIndentation,
}

#[derive(Debug)]
pub enum ErrorType {
    InternalError(InternalError),
    SyntaxError(SyntaxError),

    // To be removed
    PestSyntaxError(String),
    OldParserError(String),
}

impl From<InternalError> for ErrorType {
    fn from(e: InternalError) -> ErrorType {
        ErrorType::InternalError(e)
    }
}

impl From<SyntaxError> for ErrorType {
    fn from(e: SyntaxError) -> ErrorType {
        ErrorType::SyntaxError(e)
    }
}

#[derive(Debug)]
pub struct ParserError {
    pub error: ErrorType,
    pub span: Span,
}

impl ParserError {
    pub fn new(error: ErrorType, span: Span) -> Self {
        Self { error, span }
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ErrorType::*;

        match &self.error {
            InternalError(error) => write!(f, "Internal error {}: {}", self.span.start, error),
            SyntaxError(error) => write!(f, "Syntax error {}: {}", self.span.start, error),

            PestSyntaxError(error) => f.write_str(&error),
            OldParserError(error) => f.write_str(&error),
        }
    }
}
impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use InternalError::*;

        match self {
            AstCapacityOverflow => {
                f.write_str("There are more nodes in the program than the AST can support")
            }
            MissingScope => f.write_str("Scope unavailable during parsing"),
            NumberParseFailure => f.write_str("Failed to parse number"),
            FunctionParseFailure => f.write_str("Failed to parse function"),
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyntaxError::*;

        match self {
            UnexpectedToken => f.write_str("Unexpected Token"),
            ExpectedExpression => f.write_str("Expected expression for operation"),
            ExpectedRhsExpression => f.write_str("Expected expression for operation"),
            ExpectedCloseParen => f.write_str("Expected closing parenthesis"),
            ExpectedListEnd => f.write_str("Unexpected token while in List, expected ']'"),
            ExpectedMapSeparator => f.write_str("Expected key/value separator ':' in Map"),
            ExpectedMapValue => f.write_str("Expected value after ':' in Map"),
            ExpectedMapEnd => f.write_str("Unexpected token in Map, expected '}'"),
            ExpectedIfCondition => f.write_str("Expected condition for if statement"),
            ExpectedThenKeyword => f.write_str("Expected 'then' in if statement"),
            ExpectedThenNode => f.write_str("Expected then expression for if statement"),
            ExpectedElseNode => f.write_str("Expected else expression for if statement"),
            ExpectedAssignmentTarget => f.write_str("Expected target for assignment"),
            ExpectedFunctionArgsEnd => f.write_str("Expected end of function arguments '|'"),
            ExpectedCallArgsEnd => f.write_str("Expected end of function call arguments '|'"),
            UnexpectedIndentation => f.write_str("Unexpected indentation level"),
        }
    }
}

