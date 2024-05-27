use koto_lexer::Span;
use std::{fmt::Write, path::Path};
use thiserror::Error;

use crate::string_format_options::StringFormatError;

/// An error that represents a problem with the Parser's internal logic, rather than a user error
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum InternalError {
    #[error("There are more nodes in the program than the AST can support")]
    AstCapacityOverflow,
    #[error("There are more constants in the program than the runtime can support")]
    ConstantPoolCapacityOverflow,
    #[error("Expected ':' after map key")]
    ExpectedMapColon,
    #[error("Failed to parse ID")]
    IdParseFailure,
    #[error("Failed to parse chain")]
    ChainParseFailure,
    #[error("Missing assignment target")]
    MissingAssignmentTarget,
    #[error("Frame unavailable during parsing")]
    MissingFrame,
    #[error("Failed to parse number")]
    NumberParseFailure,
    #[error("Failed to parse raw string")]
    RawStringParseFailure,
    #[error("Unexpected token")]
    UnexpectedToken,
}

/// Errors that arise from expecting an indented block
///
/// Having these errors separated out from [SyntaxError] is useful when working with interactive
/// input, where an indented continuation can be started in response to an indentation error.
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum ExpectedIndentation {
    #[error("Expected expression after assignment operator")]
    AssignmentExpression,
    #[error("Expected indented block for catch expression")]
    CatchBody,
    #[error("Expected indented block for 'else'.")]
    ElseBlock,
    #[error("Expected indented block for 'else if'.")]
    ElseIfBlock,
    #[error("Expected indented block for finally expression")]
    FinallyBody,
    #[error("Expected indented block as for loop body")]
    ForBody,
    #[error("Expected function body")]
    FunctionBody,
    #[error("Expected indented block as loop body")]
    LoopBody,
    #[error("Expected indented arm for match expression")]
    MatchArm,
    #[error("Expected expression after binary operator")]
    RhsExpression,
    #[error("Expected indented arm for switch expression")]
    SwitchArm,
    #[error("Error parsing if expression, expected 'then' keyword or indented block.")]
    ThenKeywordOrBlock,
    #[error("Expected indented block for try expression")]
    TryBody,
    #[error("Expected indented block as until loop body")]
    UntilBody,
    #[error("Expected indented block as while loop body")]
    WhileBody,
}

/// A syntax error encountered by the [Parser]
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum SyntaxError {
    #[error("Ascii value out of range, the maximum is \\x7f")]
    AsciiEscapeCodeOutOfRange,
    #[error("Expected end of arguments ')'")]
    ExpectedArgsEnd,
    #[error("Expected target for assignment")]
    ExpectedAssignmentTarget,
    #[error("Expected '=' assignment after meta key")]
    ExpectedAssignmentAfterMetaKey,
    #[error("Expected argument for catch expression")]
    ExpectedCatchArgument,
    #[error("Expected catch expression after try")]
    ExpectedCatch,
    #[error("Expected closing parenthesis ')'")]
    ExpectedCloseParen,
    #[error("Expected expression after 'else'.")]
    ExpectedElseExpression,
    #[error("Expected condition for 'else if'.")]
    ExpectedElseIfCondition,
    #[error("Expected expression")]
    ExpectedExpression,
    #[error("Expected arguments in for loop")]
    ExpectedForArgs,
    #[error("Expected 'in' keyword in for loop")]
    ExpectedForInKeyword,
    #[error("Expected iterable in for loop")]
    ExpectedForIterable,
    #[error("Expected format string after ':'")]
    ExpectedFormatString,
    #[error("Expected end of function arguments '|'")]
    ExpectedFunctionArgsEnd,
    #[error("Expected ID in import expression")]
    ExpectedIdInImportExpression,
    #[error("Expected condition after 'if'")]
    ExpectedIfCondition,
    #[error("Expected import after from")]
    ExpectedImportAfterFrom,
    #[error("Expected module ID in import expression")]
    ExpectedImportModuleId,
    #[error("Expected index end ']'")]
    ExpectedIndexEnd,
    #[error("Expected index expression")]
    ExpectedIndexExpression,
    #[error("Expected id after 'as'")]
    ExpectedIdAfterAs,
    #[error("Expected List end ']'")]
    ExpectedListEnd,
    #[error("Expected ':' after map key")]
    ExpectedMapColon,
    #[error("Expected '}}' at end of map declaration")]
    ExpectedMapEnd,
    #[error("Expected map entry")]
    ExpectedMapEntry,
    #[error("Expected key after '.' in Map access")]
    ExpectedMapKey,
    #[error("Expected value after ':' in Map")]
    ExpectedMapValue,
    #[error("Expected expression in match arm")]
    ExpectedMatchArmExpression,
    #[error("Expected expression after then in match arm")]
    ExpectedMatchArmExpressionAfterThen,
    #[error("Expected condition after if in match arm")]
    ExpectedMatchCondition,
    #[error("Expected expression after match")]
    ExpectedMatchExpression,
    #[error("Expected pattern for match arm")]
    ExpectedMatchPattern,
    #[error("Expected id after @meta")]
    ExpectedMetaId,
    #[error("Expected a module path after 'from'")]
    ExpectedPathAfterFrom,
    #[error("Expected a line break before starting a map block")]
    ExpectedLineBreakBeforeMapBlock,
    #[error("Expected '}}' at end of string placeholder")]
    ExpectedStringPlaceholderEnd,
    #[error("Expected expression in switch arm")]
    ExpectedSwitchArmExpression,
    #[error("Expected expression after 'then' in switch arm")]
    ExpectedSwitchArmExpressionAfterThen,
    #[error("Expected a test name")]
    ExpectedTestName,
    #[error("Expected expression after 'then'")]
    ExpectedThenExpression,
    #[error("Expected condition in until loop")]
    ExpectedUntilCondition,
    #[error("Expected condition in while loop")]
    ExpectedWhileCondition,
    #[error("Expected a type after ':'")]
    ExpectedType,
    #[error(transparent)]
    FormatStringError(StringFormatError),
    #[error("Non-inline if expression isn't allowed in this context")]
    IfBlockNotAllowedInThisContext,
    #[error("Found an unexpected token while lexing input")]
    LexerError,
    #[error("Ellipsis found outside of nested match patterns")]
    MatchEllipsisOutsideOfNestedPatterns,
    #[error("'else' can only be used in the last arm in a match expression")]
    MatchElseNotInLastArm,
    #[error("Nested types aren't currently supported")]
    NestedTypesArentSupported,
    #[error("Keyword reserved for future use")]
    ReservedKeyword,
    #[error("'self' doesn't need to be declared as an argument")]
    SelfArg,
    #[error("'else' can only be used in the last arm in a switch expression")]
    SwitchElseNotInLastArm,
    #[error("Unexpected character in numeric escape code")]
    UnexpectedCharInNumericEscapeCode,
    #[error("'.' after imported item. You might want a 'from' import instead")]
    UnexpectedDotAfterImportItem,
    #[error("Unexpected escape pattern in string")]
    UnexpectedEscapeInString,
    #[error("Unexpected 'else' in match arm")]
    UnexpectedMatchElse,
    #[error("Unexpected if condition in match arm")]
    UnexpectedMatchIf,
    #[error("Unexpected meta key")]
    UnexpectedMetaKey,
    #[error("Unexpected 'else' in switch arm")]
    UnexpectedSwitchElse,
    #[error("Unexpected token")]
    UnexpectedToken,
    #[error("Unicode value out of range, the maximum is \\u{{10ffff}}")]
    UnicodeEscapeCodeOutOfRange,
    #[error("Unterminated numeric escape code")]
    UnterminatedNumericEscapeCode,
    #[error("Unterminated string")]
    UnterminatedString,
}

/// See [ParserError]
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum ErrorKind {
    #[error(transparent)]
    InternalError(#[from] InternalError),
    #[error(transparent)]
    ExpectedIndentation(#[from] ExpectedIndentation),
    #[error(transparent)]
    SyntaxError(#[from] SyntaxError),
    #[error(transparent)]
    StringFormatError(#[from] StringFormatError),
}

/// An error that can be produced by the [Parser](crate::Parser)
#[derive(Error, Clone, Debug)]
#[error("{error}")]
pub struct Error {
    /// The error itself
    pub error: ErrorKind,
    /// The span in the source string where the error occurred
    pub span: Span,
}

impl Error {
    /// Initializes a parser error with the specific error type and its associated span
    pub fn new(error: ErrorKind, span: Span) -> Self {
        Self { error, span }
    }

    /// Returns true if the error was caused by the expectation of indentation
    pub fn is_indentation_error(&self) -> bool {
        matches!(self.error, ErrorKind::ExpectedIndentation(_))
    }
}

/// The result type used by the [Parser](crate::Parser)
pub type Result<T> = std::result::Result<T, Error>;

/// Renders the excerpt of the source corresponding to the given span
pub fn format_source_excerpt(source: &str, span: &Span, source_path: Option<&Path>) -> String {
    let Span { start, end } = span;

    let (excerpt, padding) = {
        let excerpt_lines = source
            .lines()
            .skip((start.line) as usize)
            .take((end.line - start.line + 1) as usize)
            .collect::<Vec<_>>();

        let line_numbers = (start.line..=end.line)
            .map(|n| (n + 1).to_string())
            .collect::<Vec<_>>();

        let number_width = line_numbers.iter().max_by_key(|n| n.len()).unwrap().len();

        let padding = " ".repeat(number_width + 2);

        if start.line == end.line {
            let mut excerpt = format!(
                " {:>number_width$} | {}\n",
                line_numbers.first().unwrap(),
                excerpt_lines.first().unwrap(),
            );

            write!(
                excerpt,
                "{padding}|{}{}",
                " ".repeat(start.column as usize + 1),
                "^".repeat((end.column - start.column) as usize)
            )
            .ok();

            (excerpt, padding)
        } else {
            let mut excerpt = String::new();

            for (excerpt_line, line_number) in excerpt_lines.iter().zip(line_numbers.iter()) {
                writeln!(excerpt, " {line_number:>number_width$} | {excerpt_line}").ok();
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

        format!("{display_path} - {}:{}", start.line + 1, start.column + 1)
    } else {
        format!("{}:{}", start.line + 1, start.column + 1)
    };

    format!("{position_info}\n{padding}|\n{excerpt}")
}
