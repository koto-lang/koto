use {koto_lexer::Span, std::fmt};

#[derive(Debug)]
pub enum InternalError {
    ArgumentsParseFailure,
    AstCapacityOverflow,
    ExpectedIdInImportItem,
    ForParseFailure,
    FunctionParseFailure,
    IdParseFailure,
    LookupParseFailure,
    MissingAssignmentTarget,
    MissingContinuedExpressionLhs,
    MissingScope,
    NumberParseFailure,
    RangeParseFailure,
    UnexpectedIdInExpression,
    UnexpectedToken,
}

#[derive(Debug)]
pub enum SyntaxError {
    ExpectedArgsEnd,
    ExpectedAssignmentTarget,
    ExpectedCatchArgument,
    ExpectedCatchBlock,
    ExpectedCatchBody,
    ExpectedCloseParen,
    ExpectedElseBlock,
    ExpectedElseExpression,
    ExpectedElseIfBlock,
    ExpectedElseIfCondition,
    ExpectedEndOfLine,
    ExpectedExportExpression,
    ExpectedExpression,
    ExpectedExpressionInMainBlock,
    ExpectedFinallyBody,
    ExpectedForArgs,
    ExpectedForBody,
    ExpectedForCondition,
    ExpectedForInKeyword,
    ExpectedForRanges,
    ExpectedFunctionArgsEnd,
    ExpectedFunctionBody,
    ExpectedIdInImportExpression,
    ExpectedIfCondition,
    ExpectedImportKeywordAfterFrom,
    ExpectedImportModuleId,
    ExpectedIndentedLookupContinuation,
    ExpectedIndexEnd,
    ExpectedIndexExpression,
    ExpectedListEnd,
    ExpectedLoopBody,
    ExpectedMapEnd,
    ExpectedMapKey,
    ExpectedMapValue,
    ExpectedMatchArm,
    ExpectedMatchArmExpression,
    ExpectedMatchArmExpressionAfterThen,
    ExpectedMatchCondition,
    ExpectedMatchExpression,
    ExpectedMatchPattern,
    ExpectedNegatableExpression,
    ExpectedRhsExpression,
    ExpectedThenExpression,
    ExpectedThenKeywordOrBlock,
    ExpectedTryBody,
    ExpectedUntilBody,
    ExpectedUntilCondition,
    ExpectedWhileBody,
    ExpectedWhileCondition,
    ImportFromExpressionHasTooManyItems,
    LexerError,
    TooManyNum2Terms,
    TooManyNum4Terms,
    UnexpectedEscapeInString,
    UnexpectedIndentation,
    UnexpectedToken,
    UnexpectedTokenAfterExportId,
}

#[derive(Debug)]
pub enum ErrorType {
    InternalError(InternalError),
    SyntaxError(SyntaxError),
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
            InternalError(error) => write!(f, "Internal error: {}", error),
            SyntaxError(error) => write!(f, "Syntax error: {}", error),
        }
    }
}
impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use InternalError::*;

        match self {
            ArgumentsParseFailure => f.write_str("Failed to parse arguments"),
            AstCapacityOverflow => { f.write_str("There are more nodes in the program than the AST can support") }
            ExpectedIdInImportItem => f.write_str("Expected ID in import item"),
            ForParseFailure => f.write_str("Failed to parse for loop"),
            FunctionParseFailure => f.write_str("Failed to parse function"),
            IdParseFailure => f.write_str("Failed to parse ID"),
            LookupParseFailure => f.write_str("Failed to parse lookup"),
            MissingAssignmentTarget => f.write_str("Missing assignment target"),
            MissingContinuedExpressionLhs => f.write_str("Missing LHS for continued expression"),
            MissingScope => f.write_str("Scope unavailable during parsing"),
            NumberParseFailure => f.write_str("Failed to parse number"),
            RangeParseFailure => f.write_str("Failed to parse range"),
            UnexpectedIdInExpression => { f.write_str("Unexpected ID encountered while parsing expression") }
            UnexpectedToken => f.write_str("Unexpected token"),
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyntaxError::*;

        match self {
            ExpectedArgsEnd => f.write_str("Expected end of arguments ')'"),
            ExpectedAssignmentTarget => f.write_str("Expected target for assignment"),
            ExpectedCatchArgument => f.write_str("Expected argument for catch expression"),
            ExpectedCatchBlock => f.write_str("Expected catch expression after try"),
            ExpectedCatchBody => f.write_str("Expected indented block for catch expression"),
            ExpectedCloseParen => f.write_str("Expected closing parenthesis"),
            ExpectedElseBlock => f.write_str("Expected indented block for 'else'."),
            ExpectedElseExpression => f.write_str("Expected 'else' expression."),
            ExpectedElseIfBlock => f.write_str("Expected indented block for 'else if'."),
            ExpectedElseIfCondition => f.write_str("Expected condition for 'else if'."),
            ExpectedEndOfLine => f.write_str("Expected end of line"),
            ExpectedExportExpression => f.write_str("Expected ID to export"),
            ExpectedExpression => f.write_str("Expected expression"),
            ExpectedExpressionInMainBlock => f.write_str("Expected expression in main block"),
            ExpectedFinallyBody => f.write_str("Expected indented block for finally expression"),
            ExpectedForArgs => f.write_str("Expected arguments in for loop"),
            ExpectedForBody => f.write_str("Expected indented block in for loop"),
            ExpectedForCondition => f.write_str("Expected condition after 'if' in for loop"),
            ExpectedForInKeyword => f.write_str("Expected in keyword in for loop"),
            ExpectedForRanges => f.write_str("Expected ranges in for loop"),
            ExpectedFunctionArgsEnd => f.write_str("Expected end of function arguments '|'"),
            ExpectedFunctionBody => f.write_str("Expected function body"),
            ExpectedIdInImportExpression => f.write_str("Expected ID in import expression"),
            ExpectedIfCondition => f.write_str("Expected condition in if expression"),
            ExpectedImportKeywordAfterFrom => f.write_str("Expected 'import' after 'from' ID"),
            ExpectedImportModuleId => f.write_str("Expected module ID in import expression"),
            ExpectedIndentedLookupContinuation => { f.write_str("Expected indented lookup continuation") }
            ExpectedIndexEnd => f.write_str("Unexpected token while indexing a List, expected ']'"),
            ExpectedIndexExpression => f.write_str("Expected index expression"),
            ExpectedListEnd => f.write_str("Unexpected token while in List, expected ']'"),
            ExpectedLoopBody => f.write_str("Expected indented block in loop"),
            ExpectedMapEnd => f.write_str("Unexpected token in Map, expected '}'"),
            ExpectedMapKey => f.write_str("Expected key after '.' in Map access"),
            ExpectedMapValue => f.write_str("Expected value after ':' in Map"),
            ExpectedMatchArm => f.write_str("Expected indented arm for match expression"),
            ExpectedMatchArmExpression => f.write_str("Expected expression in match arm"),
            ExpectedMatchArmExpressionAfterThen => { f.write_str("Expected expression after then in match arm") }
            ExpectedMatchCondition => f.write_str("Expected condition after if in match arm"),
            ExpectedMatchExpression => f.write_str("Expected expression after match"),
            ExpectedMatchPattern => f.write_str("Expected pattern for match arm"),
            ExpectedNegatableExpression => f.write_str("Expected negatable expression"),
            ExpectedRhsExpression => f.write_str("Expected expression"),
            ExpectedThenExpression => f.write_str("Expected 'then' expression."),
            ExpectedThenKeywordOrBlock => f.write_str( "Error parsing if expression, expected 'then' keyword or indented block.",),
            ExpectedTryBody => f.write_str("Expected indented block for try expression"),
            ExpectedUntilBody => f.write_str("Expected indented block in until loop"),
            ExpectedUntilCondition => f.write_str("Expected condition in until loop"),
            ExpectedWhileBody => f.write_str("Expected indented block in while loop"),
            ExpectedWhileCondition => f.write_str("Expected condition in while loop"),
            ImportFromExpressionHasTooManyItems => { f.write_str("Too many items listed after 'from' in import expression") }
            LexerError => f.write_str("Found an unexpected token while lexing input"),
            TooManyNum2Terms => f.write_str("num2 only supports up to 2 terms"),
            TooManyNum4Terms => f.write_str("num4 only supports up to 4 terms"),
            UnexpectedEscapeInString => f.write_str("Unexpected escape pattern in string"),
            UnexpectedIndentation => f.write_str("Unexpected indentation level"),
            UnexpectedToken => f.write_str("Unexpected token"),
            UnexpectedTokenAfterExportId => f.write_str("Unexpected token after export ID"),
        }
    }
}
