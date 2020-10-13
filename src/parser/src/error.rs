use {koto_lexer::Span, std::fmt};

#[derive(Debug)]
pub enum InternalError {
    UnexpectedToken,
    AstCapacityOverflow,
    MissingScope,
    NumberParseFailure,
    IdParseFailure,
    FunctionParseFailure,
    RangeParseFailure,
    ForParseFailure,
    ArgumentsParseFailure,
    MissingContinuedExpressionLhs,
    MissingAssignmentTarget,
    UnexpectedIdInExpression,
    ExpectedIdInImportItem,
}

#[derive(Debug)]
pub enum SyntaxError {
    LexerError,
    UnexpectedToken,
    UnexpectedIndentation,
    ExpectedEndOfLine,
    ExpectedExpression,
    ExpectedExpressionInMainBlock,
    ExpectedRhsExpression,
    ExpectedCloseParen,
    ExpectedListEnd,
    ExpectedIndexEnd,
    ExpectedIndexExpression,
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
    ExpectedMatchExpression,
    ExpectedMatchArm,
    ExpectedMatchPattern,
    ExpectedMatchCondition,
    ExpectedMatchArmExpression,
    ExpectedMatchArmExpressionAfterThen,
    ExpectedAssignmentTarget,
    ExpectedFunctionArgsEnd,
    ExpectedFunctionBody,
    ExpectedArgsEnd,
    ExpectedForArgs,
    ExpectedForInKeyword,
    ExpectedForRanges,
    ExpectedForCondition,
    ExpectedForBody,
    ExpectedWhileCondition,
    ExpectedWhileBody,
    ExpectedUntilCondition,
    ExpectedUntilBody,
    ExpectedLoopBody,
    ExpectedExportExpression,
    ExpectedNegatableExpression,
    ExpectedImportModuleId,
    ExpectedIdInImportExpression,
    ImportFromExpressionHasTooManyItems,
    ExpectedImportKeywordAfterFrom,
    UnexpectedTokenAfterExportId,
    TooManyNum2Terms,
    TooManyNum4Terms,
    ExpectedTryBody,
    ExpectedCatchBlock,
    ExpectedCatchBody,
    ExpectedCatchArgument,
    ExpectedFinallyBody,
    UnexpectedEscapeInString,
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
            UnexpectedToken => f.write_str("Unexpected token"),
            AstCapacityOverflow => {
                f.write_str("There are more nodes in the program than the AST can support")
            }
            MissingScope => f.write_str("Scope unavailable during parsing"),
            NumberParseFailure => f.write_str("Failed to parse number"),
            IdParseFailure => f.write_str("Failed to parse ID"),
            FunctionParseFailure => f.write_str("Failed to parse function"),
            RangeParseFailure => f.write_str("Failed to parse range"),
            ForParseFailure => f.write_str("Failed to parse for loop"),
            ArgumentsParseFailure => f.write_str("Failed to parse arguments"),
            MissingContinuedExpressionLhs => f.write_str("Missing LHS for continued expression"),
            MissingAssignmentTarget => f.write_str("Missing assignment target"),
            UnexpectedIdInExpression => {
                f.write_str("Unexpected ID encountered while parsing expression")
            }
            ExpectedIdInImportItem => f.write_str("Expected ID in import item"),
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyntaxError::*;

        match self {
            LexerError => f.write_str("Found an unexpected token while lexing input"),
            UnexpectedToken => f.write_str("Unexpected token"),
            UnexpectedIndentation => f.write_str("Unexpected indentation level"),
            ExpectedEndOfLine => f.write_str("Expected end of line"),
            ExpectedExpression => f.write_str("Expected expression"),
            ExpectedExpressionInMainBlock => f.write_str("Expected expression in main block"),
            ExpectedRhsExpression => f.write_str("Expected expression"),
            ExpectedCloseParen => f.write_str("Expected closing parenthesis"),
            ExpectedListEnd => f.write_str("Unexpected token while in List, expected ']'"),
            ExpectedIndexEnd => f.write_str("Unexpected token while indexing a List, expected ']'"),
            ExpectedIndexExpression => f.write_str("Expected index expression"),
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
            ExpectedMatchExpression => f.write_str("Expected expression after match"),
            ExpectedMatchArm => f.write_str("Expected indented arm for match expression"),
            ExpectedMatchPattern => f.write_str("Expected pattern for match arm"),
            ExpectedMatchCondition => f.write_str("Expected condition after if in match arm"),
            ExpectedMatchArmExpression => f.write_str("Expected expression in match arm"),
            ExpectedMatchArmExpressionAfterThen => {
                f.write_str("Expected expression after then in match arm")
            }
            ExpectedAssignmentTarget => f.write_str("Expected target for assignment"),
            ExpectedFunctionArgsEnd => f.write_str("Expected end of function arguments '|'"),
            ExpectedFunctionBody => f.write_str("Expected function body"),
            ExpectedArgsEnd => f.write_str("Expected end of arguments ')'"),
            ExpectedForArgs => f.write_str("Expected arguments in for loop"),
            ExpectedForInKeyword => f.write_str("Expected in keyword in for loop"),
            ExpectedForRanges => f.write_str("Expected ranges in for loop"),
            ExpectedForCondition => f.write_str("Expected condition after 'if' in for loop"),
            ExpectedForBody => f.write_str("Expected indented block in for loop"),
            ExpectedWhileCondition => f.write_str("Expected condition in while loop"),
            ExpectedWhileBody => f.write_str("Expected indented block in while loop"),
            ExpectedUntilCondition => f.write_str("Expected condition in until loop"),
            ExpectedUntilBody => f.write_str("Expected indented block in until loop"),
            ExpectedLoopBody => f.write_str("Expected indented block in loop"),
            ExpectedExportExpression => f.write_str("Expected ID to export"),
            ExpectedNegatableExpression => f.write_str("Expected negatable expression"),
            ExpectedIdInImportExpression => f.write_str("Expected ID in import expression"),
            ExpectedImportKeywordAfterFrom => f.write_str("Expected 'import' after 'from' ID"),
            ExpectedImportModuleId => f.write_str("Expected module ID in import expression"),
            ImportFromExpressionHasTooManyItems => {
                f.write_str("Too many items listed after 'from' in import expression")
            }
            UnexpectedTokenAfterExportId => f.write_str("Unexpected token after export ID"),
            TooManyNum2Terms => f.write_str("num2 only supports up to 2 terms"),
            TooManyNum4Terms => f.write_str("num4 only supports up to 4 terms"),
            ExpectedTryBody => f.write_str("Expected indented block for try expression"),
            ExpectedCatchBlock => f.write_str("Expected catch expression after try"),
            ExpectedCatchBody => f.write_str("Expected indented block for catch expression"),
            ExpectedCatchArgument => f.write_str("Expected argument for catch expression"),
            ExpectedFinallyBody => f.write_str("Expected indented block for finally expression"),
            UnexpectedEscapeInString => f.write_str("Unexpected escape pattern in string"),
        }
    }
}
