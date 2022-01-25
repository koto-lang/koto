use {
    koto_lexer::{Position, Span},
    std::{error, fmt, path::PathBuf},
};

/// An error that represents a problem with the Parser's internal logic, rather than a user error
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum InternalError {
    ArgumentsParseFailure,
    AstCapacityOverflow,
    ConstantPoolCapacityOverflow,
    ExpectedIdInImportItem,
    ExpectedMapColon,
    ForParseFailure,
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

/// Errors that arise from expecting an indented block
///
/// Having these errors separated out from [SyntaxError] is useful when working with interactive
/// input, where an indented continuation can be started in response to an indentation error.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum ExpectedIndentation {
    CatchBody,
    ElseBlock,
    ElseIfBlock,
    FinallyBody,
    ForBody,
    FunctionBody,
    LoopBody,
    MatchArm,
    RhsExpression,
    SwitchArm,
    ThenKeywordOrBlock,
    TryBody,
    UntilBody,
    WhileBody,
}

/// A syntax error encountered by the [Parser]
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum SyntaxError {
    AsciiEscapeCodeOutOfRange,
    ExpectedArgsEnd,
    ExpectedAssignmentTarget,
    ExpectedAssignmentAfterMetaKey,
    ExpectedCatchArgument,
    ExpectedCatch,
    ExpectedCloseParen,
    ExpectedElseExpression,
    ExpectedElseIfCondition,
    ExpectedEndOfLine,
    ExpectedExportExpression,
    ExpectedExpression,
    ExpectedExpressionInMainBlock,
    ExpectedForArgs,
    ExpectedForCondition,
    ExpectedForInKeyword,
    ExpectedForIterable,
    ExpectedFunctionArgsEnd,
    ExpectedIdInImportExpression,
    ExpectedIfCondition,
    ExpectedImportKeywordAfterFrom,
    ExpectedImportModuleId,
    ExpectedIndentedLookupContinuation,
    ExpectedIndexEnd,
    ExpectedIndexExpression,
    ExpectedListEnd,
    ExpectedMapColon,
    ExpectedMapEnd,
    ExpectedMapEntry,
    ExpectedMapKey,
    ExpectedMapValue,
    ExpectedMatchArmExpression,
    ExpectedMatchArmExpressionAfterThen,
    ExpectedMatchCondition,
    ExpectedMatchExpression,
    ExpectedMatchPattern,
    ExpectedMetaKey,
    ExpectedMetaId,
    ExpectedNegatableExpression,
    ExpectedSwitchArmExpression,
    ExpectedSwitchArmExpressionAfterThen,
    ExpectedStringPlaceholderEnd,
    ExpectedThenExpression,
    ExpectedTestName,
    ExpectedUntilCondition,
    ExpectedWhileCondition,
    IfBlockNotAllowedInThisContext,
    ImportFromExpressionHasTooManyItems,
    LexerError,
    MatchEllipsisOutsideOfNestedPatterns,
    MatchElseNotInLastArm,
    SelfArgNotInFirstPosition,
    SwitchElseNotInLastArm,
    UnexpectedCharInNumericEscapeCode,
    UnexpectedElseIndentation,
    UnexpectedElseIfIndentation,
    UnexpectedEscapeInString,
    UnexpectedExportAssignmentOp,
    UnexpectedMatchElse,
    UnexpectedMatchIf,
    UnexpectedMetaKey,
    UnexpectedSwitchElse,
    UnexpectedToken,
    UnexpectedTokenAfterDollarInString,
    UnexpectedTokenInImportExpression,
    UnicodeEscapeCodeOutOfRange,
    UnnecessaryExportKeywordForMetaKey,
    UnterminatedNumericEscapeCode,
    UnterminatedString,
}

/// See [ParserError]
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum ErrorType {
    InternalError(InternalError),
    ExpectedIndentation(ExpectedIndentation),
    SyntaxError(SyntaxError),
}

impl From<InternalError> for ErrorType {
    fn from(e: InternalError) -> ErrorType {
        ErrorType::InternalError(e)
    }
}

impl From<ExpectedIndentation> for ErrorType {
    fn from(e: ExpectedIndentation) -> ErrorType {
        ErrorType::ExpectedIndentation(e)
    }
}

impl From<SyntaxError> for ErrorType {
    fn from(e: SyntaxError) -> ErrorType {
        ErrorType::SyntaxError(e)
    }
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ErrorType::*;

        match &self {
            InternalError(error) => write!(f, "Internal error: {}", error),
            ExpectedIndentation(error) => f.write_str(&error.to_string()),
            SyntaxError(error) => f.write_str(&error.to_string()),
        }
    }
}

/// An error that can be produced by the [Parser](crate::Parser)
#[derive(Clone, Debug)]
pub struct ParserError {
    /// The error itself
    pub error: ErrorType,
    /// The span in the source string where the error occurred
    pub span: Span,
}

impl ParserError {
    /// Initializes a parser error with the specific error type and its associated span
    pub fn new(error: ErrorType, span: Span) -> Self {
        Self { error, span }
    }

    /// Returns true if the error was caused by the expectation of indentation
    pub fn is_indentation_error(&self) -> bool {
        matches!(self.error, ErrorType::ExpectedIndentation(_))
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl error::Error for ParserError {}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use InternalError::*;

        match self {
            ArgumentsParseFailure => f.write_str("Failed to parse arguments"),
            AstCapacityOverflow => {
                f.write_str("There are more nodes in the program than the AST can support")
            }
            ConstantPoolCapacityOverflow => {
                f.write_str("There are more constants in the program than the runtime can support")
            }
            ExpectedIdInImportItem => f.write_str("Expected ID in import item"),
            ExpectedMapColon => f.write_str("Expected ':' after map key"),
            ForParseFailure => f.write_str("Failed to parse for loop"),
            IdParseFailure => f.write_str("Failed to parse ID"),
            LookupParseFailure => f.write_str("Failed to parse lookup"),
            MissingAssignmentTarget => f.write_str("Missing assignment target"),
            MissingContinuedExpressionLhs => f.write_str("Missing LHS for continued expression"),
            MissingScope => f.write_str("Scope unavailable during parsing"),
            NumberParseFailure => f.write_str("Failed to parse number"),
            RangeParseFailure => f.write_str("Failed to parse range"),
            UnexpectedIdInExpression => {
                f.write_str("Unexpected ID encountered while parsing expression")
            }
            UnexpectedToken => f.write_str("Unexpected token"),
        }
    }
}

impl fmt::Display for ExpectedIndentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExpectedIndentation::*;

        match self {
            CatchBody => f.write_str("Expected indented block for catch expression"),
            ElseBlock => f.write_str("Expected indented block for 'else'."),
            ElseIfBlock => f.write_str("Expected indented block for 'else if'."),
            ForBody => f.write_str("Expected indented block in for loop"),
            FinallyBody => f.write_str("Expected indented block for finally expression"),
            FunctionBody => f.write_str("Expected function body"),
            LoopBody => f.write_str("Expected indented block in loop"),
            MatchArm => f.write_str("Expected indented arm for match expression"),
            SwitchArm => f.write_str("Expected indented arm for switch expression"),
            RhsExpression => f.write_str("Expected expression"),
            ThenKeywordOrBlock => f.write_str(
                "Error parsing if expression, expected 'then' keyword or indented block.",
            ),
            TryBody => f.write_str("Expected indented block for try expression"),
            UntilBody => f.write_str("Expected indented block in until loop"),
            WhileBody => f.write_str("Expected indented block in while loop"),
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyntaxError::*;

        match self {
            AsciiEscapeCodeOutOfRange => {
                f.write_str("Ascii value out of range, the maximum is \\x7f")
            }
            ExpectedArgsEnd => f.write_str("Expected end of arguments ')'"),
            ExpectedAssignmentTarget => f.write_str("Expected target for assignment"),
            ExpectedAssignmentAfterMetaKey => f.write_str("Expected '=' assignment after meta key"),
            ExpectedCatchArgument => f.write_str("Expected argument for catch expression"),
            ExpectedCatch => f.write_str("Expected catch expression after try"),
            ExpectedCloseParen => f.write_str("Expected closing parenthesis ')'"),
            ExpectedElseExpression => f.write_str("Expected 'else' expression."),
            ExpectedElseIfCondition => f.write_str("Expected condition for 'else if'."),
            ExpectedEndOfLine => f.write_str("Expected end of line"),
            ExpectedExportExpression => f.write_str("Expected ID to export"),
            ExpectedExpression => f.write_str("Expected expression"),
            ExpectedExpressionInMainBlock => f.write_str("Expected expression"),
            ExpectedForArgs => f.write_str("Expected arguments in for loop"),
            ExpectedForCondition => f.write_str("Expected condition after 'if' in for loop"),
            ExpectedForInKeyword => f.write_str("Expected in keyword in for loop"),
            ExpectedForIterable => f.write_str("Expected iterable in for loop"),
            ExpectedFunctionArgsEnd => f.write_str("Expected end of function arguments '|'"),
            ExpectedIdInImportExpression => f.write_str("Expected ID in import expression"),
            ExpectedIfCondition => f.write_str("Expected condition in if expression"),
            ExpectedImportKeywordAfterFrom => f.write_str("Expected 'import' after 'from' ID"),
            ExpectedImportModuleId => f.write_str("Expected module ID in import expression"),
            ExpectedIndentedLookupContinuation => {
                f.write_str("Expected indented lookup continuation")
            }
            ExpectedIndexEnd => f.write_str("Expected index end ']'"),
            ExpectedIndexExpression => f.write_str("Expected index expression"),
            ExpectedListEnd => f.write_str("Expected List end ']'"),
            ExpectedMapColon => f.write_str("Expected ':' after map key"),
            ExpectedMapEnd => f.write_str("Unexpected token in Map, expected '}'"),
            ExpectedMapEntry => f.write_str("Expected map entry"),
            ExpectedMapKey => f.write_str("Expected key after '.' in Map access"),
            ExpectedMapValue => f.write_str("Expected value after ':' in Map"),
            ExpectedMatchArmExpression => f.write_str("Expected expression in match arm"),
            ExpectedMatchArmExpressionAfterThen => {
                f.write_str("Expected expression after then in match arm")
            }
            ExpectedMatchCondition => f.write_str("Expected condition after if in match arm"),
            ExpectedMatchExpression => f.write_str("Expected expression after match"),
            ExpectedMatchPattern => f.write_str("Expected pattern for match arm"),
            ExpectedMetaKey => f.write_str("Expected meta key after @"),
            ExpectedMetaId => f.write_str("Expected id after @meta"),
            ExpectedNegatableExpression => f.write_str("Expected negatable expression"),
            ExpectedStringPlaceholderEnd => {
                f.write_str("Expected '}' at end of string placeholder")
            }
            ExpectedSwitchArmExpression => f.write_str("Expected expression in switch arm"),
            ExpectedSwitchArmExpressionAfterThen => {
                f.write_str("Expected expression after then in switch arm")
            }
            ExpectedTestName => f.write_str("Expected a test name"),
            ExpectedThenExpression => f.write_str("Expected 'then' expression."),
            ExpectedUntilCondition => f.write_str("Expected condition in until loop"),
            ExpectedWhileCondition => f.write_str("Expected condition in while loop"),
            IfBlockNotAllowedInThisContext => {
                f.write_str("Non-inline if expression isn't allowed in this context.")
            }
            ImportFromExpressionHasTooManyItems => {
                f.write_str("Too many items listed after 'from' in import expression")
            }
            LexerError => f.write_str("Found an unexpected token while lexing input"),
            MatchEllipsisOutsideOfNestedPatterns => {
                f.write_str("Ellipsis found outside of nested match patterns")
            }
            MatchElseNotInLastArm => {
                f.write_str("else can only be used in the last arm in a match expression")
            }
            SwitchElseNotInLastArm => {
                f.write_str("else can only be used in the last arm in a switch expression")
            }
            SelfArgNotInFirstPosition => f.write_str("self is only allowed as the first argument"),
            UnexpectedCharInNumericEscapeCode => {
                f.write_str("Unexpected character in numeric escape code")
            }
            UnexpectedElseIndentation => f.write_str("Unexpected indentation for else block"),
            UnexpectedElseIfIndentation => f.write_str("Unexpected indentation for else if block"),
            UnexpectedEscapeInString => f.write_str("Unexpected escape pattern in string"),
            UnexpectedExportAssignmentOp => {
                f.write_str("Unexpected assignment op for export expression (expected '=')")
            }
            UnexpectedMatchElse => f.write_str("Unexpected else in match arm"),
            UnexpectedMatchIf => f.write_str("Unexpected if condition in match arm"),
            UnexpectedMetaKey => f.write_str("Unexpected meta key"),
            UnexpectedSwitchElse => f.write_str("Unexpected else in switch arm"),
            UnexpectedToken => f.write_str("Unexpected token"),
            UnexpectedTokenAfterDollarInString => {
                f.write_str("Unexpected token after $ in string, expected $ID or ${expression}")
            }
            UnexpectedTokenInImportExpression => {
                f.write_str("Unexpected token in import expression")
            }
            UnicodeEscapeCodeOutOfRange => {
                f.write_str("Unicode value out of range, the maximum is \\u{10ffff}")
            }
            UnnecessaryExportKeywordForMetaKey => {
                f.write_str("'export' is unnecessary when assigning to a meta key")
            }
            UnterminatedNumericEscapeCode => f.write_str("Unterminated numeric escape code"),
            UnterminatedString => f.write_str("Unterminated string"),
        }
    }
}

/// Produces a formatted error string with a corresponding source excerpt
pub fn format_error_with_excerpt(
    message: Option<&str>,
    source_path: &Option<PathBuf>,
    source: &str,
    start_pos: Position,
    end_pos: Position,
) -> String {
    let (excerpt, padding) = {
        let excerpt_lines = source
            .lines()
            .skip((start_pos.line - 1) as usize)
            .take((end_pos.line - start_pos.line + 1) as usize)
            .collect::<Vec<_>>();

        let line_numbers = (start_pos.line..=end_pos.line)
            .map(|n| n.to_string())
            .collect::<Vec<_>>();

        let number_width = line_numbers.iter().max_by_key(|n| n.len()).unwrap().len();

        let padding = " ".repeat(number_width + 2);

        if start_pos.line == end_pos.line {
            let mut excerpt = format!(
                " {:>width$} | {}\n",
                line_numbers.first().unwrap(),
                excerpt_lines.first().unwrap(),
                width = number_width
            );

            excerpt += &format!(
                "{}|{}{}",
                padding,
                " ".repeat(start_pos.column as usize),
                "^".repeat((end_pos.column - start_pos.column) as usize)
            );

            (excerpt, padding)
        } else {
            let mut excerpt = String::new();

            for (excerpt_line, line_number) in excerpt_lines.iter().zip(line_numbers.iter()) {
                excerpt += &format!(
                    " {:>width$} | {}\n",
                    line_number,
                    excerpt_line,
                    width = number_width
                );
            }

            (excerpt, padding)
        }
    };

    let position_info = if let Some(path) = source_path {
        let display_path = if let Ok(current_dir) = std::env::current_dir() {
            if let Ok(stripped) = path.strip_prefix(current_dir) {
                stripped.display()
            } else {
                path.display()
            }
        } else {
            path.display()
        };

        format!("{} - {}:{}", display_path, start_pos.line, start_pos.column)
    } else {
        format!("{}:{}", start_pos.line, start_pos.column)
    };

    format!(
        "{message}\n --- {position_info}\n{padding}|\n{excerpt}",
        message = message.unwrap_or(""),
    )
}
