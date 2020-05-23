use {koto_lexer::Span, std::fmt};

#[derive(Debug)]
pub enum InternalError {
    AstCapacityOverflow,
    MissingScope,
    NumberParseFailure,
    FunctionParseFailure,
    RangeParseFailure,
    ForParseFailure,
    MissingLookupId,
    MissingContinuedExpressionLhs,
    MissingAssignmentTarget,
}

#[derive(Debug)]
pub enum SyntaxError {
    UnexpectedToken,
    UnexpectedIndentation,
    ExpectedEndOfLine,
    ExpectedExpression,
    ExpectedRhsExpression,
    ExpectedCloseParen,
    ExpectedListEnd,
    ExpectedIndexEnd,
    ExpectedIndexExpression,
    ExpectedMapSeparator,
    ExpectedMapKey,
    ExpectedMapValue,
    ExpectedMapEnd,
    ExpectedIfCondition,
    ExpectedThenKeywordOrBlock,
    ExpectedThenExpression,
    ExpectedElseIfCondition,
    ExpectedElseIfBlock,
    ExpectedElseExpression,
    ExpectedElseBlock,
    ExpectedAssignmentTarget,
    ExpectedFunctionArgsEnd,
    ExpectedFunctionBody,
    ExpectedCallArgsEnd,
    ExpectedForArgs,
    ExpectedForInKeyword,
    ExpectedForRanges,
    ExpectedForCondition,
    ExpectedForBody,
    ExpectedWhileCondition,
    ExpectedWhileBody,
    ExpectedUntilCondition,
    ExpectedUntilBody,
    ExpectedExportExpression,
    UnexpectedTokenAfterExportId,
    TooManyNum2Terms,
    TooManyNum4Terms,
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
            RangeParseFailure => f.write_str("Failed to parse range"),
            ForParseFailure => f.write_str("Failed to parse for loop"),
            MissingLookupId => f.write_str("Missing lookup Id"),
            MissingContinuedExpressionLhs => f.write_str("Missing LHS for continued expression"),
            MissingAssignmentTarget => f.write_str("Missing assignment target"),
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyntaxError::*;

        match self {
            UnexpectedToken => f.write_str("Unexpected Token"),
            UnexpectedIndentation => f.write_str("Unexpected indentation level"),
            ExpectedEndOfLine => f.write_str("Expected end of line"),
            ExpectedExpression => f.write_str("Expected expression"),
            ExpectedRhsExpression => f.write_str("Expected expression"),
            ExpectedCloseParen => f.write_str("Expected closing parenthesis"),
            ExpectedListEnd => f.write_str("Unexpected token while in List, expected ']'"),
            ExpectedIndexEnd => f.write_str("Unexpected token while indexing a List, expected ']'"),
            ExpectedIndexExpression => f.write_str("Expected index expression"),
            ExpectedMapSeparator => f.write_str("Expected key/value separator ':' in Map"),
            ExpectedMapKey => f.write_str("Expected key after '.' in Map access"),
            ExpectedMapValue => f.write_str("Expected value after ':' in Map"),
            ExpectedMapEnd => f.write_str("Unexpected token in Map, expected '}'"),
            ExpectedIfCondition => f.write_str("Expected condition in if expression"),
            ExpectedThenKeywordOrBlock => f.write_str(
                "Error parsing if expression, expected 'then' keyword or indented block.",
            ),
            ExpectedThenExpression => f.write_str("Expected 'then' expression."),
            ExpectedElseIfCondition => f.write_str("Expected condition for 'else if'."),
            ExpectedElseIfBlock => f.write_str("Expected indented block for 'else if'."),
            ExpectedElseExpression => f.write_str("Expected 'else' expression."),
            ExpectedElseBlock => f.write_str("Expected indented block for 'else'."),
            ExpectedAssignmentTarget => f.write_str("Expected target for assignment"),
            ExpectedFunctionArgsEnd => f.write_str("Expected end of function arguments '|'"),
            ExpectedFunctionBody => f.write_str("Expected function body"),
            ExpectedCallArgsEnd => f.write_str("Expected end of function call arguments '|'"),
            ExpectedForArgs => f.write_str("Expected arguments in for loop"),
            ExpectedForInKeyword => f.write_str("Expected in keyword in for loop"),
            ExpectedForRanges => f.write_str("Expected ranges in for loop"),
            ExpectedForCondition => f.write_str("Expected condition after 'if' in for loop"),
            ExpectedForBody => f.write_str("Expected indented block in for loop"),
            ExpectedWhileCondition => f.write_str("Expected condition in while loop"),
            ExpectedWhileBody => f.write_str("Expected indented block in while loop"),
            ExpectedUntilCondition => f.write_str("Expected condition in until loop"),
            ExpectedUntilBody => f.write_str("Expected indented block in until loop"),
            ExpectedExportExpression => f.write_str("Expected id to export"),
            UnexpectedTokenAfterExportId => f.write_str("Unexpected token after export id"),
            TooManyNum2Terms => f.write_str("num2 only supports up to 2 terms"),
            TooManyNum4Terms => f.write_str("num4 only supports up to 4 terms"),
        }
    }
}
