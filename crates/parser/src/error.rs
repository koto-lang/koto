use koto_lexer::Span;
use std::fmt::Write;
use thiserror::Error;

use crate::string_format_options::StringFormatError;

/// An error that represents a problem with the Parser's internal logic, rather than a user error
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum InternalError {
    #[error("there are more nodes in the program than the AST can support")]
    AstCapacityOverflow,
    #[error("there are more constants in the program than the runtime can support")]
    ConstantPoolCapacityOverflow,
    #[error("expected ':' after map key")]
    ExpectedMapColon,
    #[error("failed to parse ID")]
    IdParseFailure,
    #[error("failed to parse chain")]
    ChainParseFailure,
    #[error("missing assignment target")]
    MissingAssignmentTarget,
    #[error("frame unavailable during parsing")]
    MissingFrame,
    #[error("failed to parse number")]
    NumberParseFailure,
    #[error("failed to parse raw string")]
    RawStringParseFailure,
    #[error("unexpected token")]
    UnexpectedToken,
}

/// Errors that arise from expecting an indented block
///
/// Having these errors separated out from [SyntaxError] is useful when working with interactive
/// input, where an indented continuation can be started in response to an indentation error.
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum ExpectedIndentation {
    #[error("expected expression after assignment operator")]
    AssignmentExpression,
    #[error("expected indented block for catch expression")]
    CatchBody,
    #[error("expected indented block for 'else'.")]
    ElseBlock,
    #[error("expected indented block for 'else if'.")]
    ElseIfBlock,
    #[error("expected indented block for finally expression")]
    FinallyBody,
    #[error("expected indented block as for loop body")]
    ForBody,
    #[error("expected function body")]
    FunctionBody,
    #[error("expected indented block as loop body")]
    LoopBody,
    #[error("expected indented arm for match expression")]
    MatchArm,
    #[error("expected expression after binary operator")]
    RhsExpression,
    #[error("expected indented arm for switch expression")]
    SwitchArm,
    #[error("error parsing if expression, expected 'then' keyword or indented block.")]
    ThenKeywordOrBlock,
    #[error("expected indented block for try expression")]
    TryBody,
    #[error("expected indented block as until loop body")]
    UntilBody,
    #[error("expected indented block as while loop body")]
    WhileBody,
}

/// A syntax error encountered by the [Parser]
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum SyntaxError {
    #[error("ascii value out of range, the maximum is \\x7f")]
    AsciiEscapeCodeOutOfRange,
    #[error("expected end of arguments ')'")]
    ExpectedArgsEnd,
    #[error("expected target for assignment")]
    ExpectedAssignmentTarget,
    #[error("expected '=' assignment after meta key")]
    ExpectedAssignmentAfterMetaKey,
    #[error("expected argument for catch expression")]
    ExpectedCatchArgument,
    #[error("expected catch expression after try")]
    ExpectedCatch,
    #[error("expected closing parenthesis ')'")]
    ExpectedCloseParen,
    #[error("all arguments following a default value must also have a default value")]
    ExpectedDefaultValue,
    #[error("expected expression after 'else'.")]
    ExpectedElseExpression,
    #[error("expected condition for 'else if'.")]
    ExpectedElseIfCondition,
    #[error("expected expression")]
    ExpectedExpression,
    #[error("expected arguments in for loop")]
    ExpectedForArgs,
    #[error("expected 'in' keyword in for loop")]
    ExpectedForInKeyword,
    #[error("expected iterable in for loop")]
    ExpectedForIterable,
    #[error("expected format string after ':'")]
    ExpectedFormatString,
    #[error("expected end of function arguments '|'")]
    ExpectedFunctionArgsEnd,
    #[error("expected ID in import expression")]
    ExpectedIdInImportExpression,
    #[error("expected condition after 'if'")]
    ExpectedIfCondition,
    #[error("expected import after from")]
    ExpectedImportAfterFrom,
    #[error("expected module ID in import expression")]
    ExpectedImportModuleId,
    #[error("expected index end ']'")]
    ExpectedIndexEnd,
    #[error("expected index expression")]
    ExpectedIndexExpression,
    #[error("expected id after 'as'")]
    ExpectedIdAfterAs,
    #[error("expected List end ']'")]
    ExpectedListEnd,
    #[error("expected ':' after map key")]
    ExpectedMapColon,
    #[error("expected '}}' at end of map declaration")]
    ExpectedMapEnd,
    #[error("expected map entry")]
    ExpectedMapEntry,
    #[error("expected key after '.' in Map access")]
    ExpectedMapKey,
    #[error("expected value after ':' in Map")]
    ExpectedMapValue,
    #[error("expected expression in match arm")]
    ExpectedMatchArmExpression,
    #[error("expected expression after then in match arm")]
    ExpectedMatchArmExpressionAfterThen,
    #[error("expected condition after if in match arm")]
    ExpectedMatchCondition,
    #[error("expected expression after match")]
    ExpectedMatchExpression,
    #[error("expected pattern for match arm")]
    ExpectedMatchPattern,
    #[error("expected id after @meta")]
    ExpectedMetaId,
    #[error("expected a module path after 'from'")]
    ExpectedPathAfterFrom,
    #[error("expected a line break before starting a map block")]
    ExpectedLineBreakBeforeMapBlock,
    #[error("expected '}}' at end of string placeholder")]
    ExpectedStringPlaceholderEnd,
    #[error("expected expression in switch arm")]
    ExpectedSwitchArmExpression,
    #[error("expected expression after 'then' in switch arm")]
    ExpectedSwitchArmExpressionAfterThen,
    #[error("expected a test name")]
    ExpectedTestName,
    #[error("expected expression after 'then'")]
    ExpectedThenExpression,
    #[error("expected condition in until loop")]
    ExpectedUntilCondition,
    #[error("expected condition in while loop")]
    ExpectedWhileCondition,
    #[error("expected a type after ':'")]
    ExpectedType,
    #[error(transparent)]
    FormatStringError(StringFormatError),
    #[error("non-inline if expression isn't allowed in this context")]
    IfBlockNotAllowedInThisContext,
    #[error("ellipsis found outside of nested match patterns")]
    MatchEllipsisOutsideOfNestedPatterns,
    #[error("'else' can only be used in the last arm in a match expression")]
    MatchElseNotInLastArm,
    #[error("nested types aren't currently supported")]
    NestedTypesArentSupported,
    #[error("keyword reserved for future use")]
    ReservedKeyword,
    #[error("'self' doesn't need to be declared as an argument")]
    SelfArg,
    #[error("'else' can only be used in the last arm in a switch expression")]
    SwitchElseNotInLastArm,
    #[error("unexpected character in numeric escape code")]
    UnexpectedCharInNumericEscapeCode,
    #[error("'.' after imported item. You might want a 'from' import instead")]
    UnexpectedDotAfterImportItem,
    #[error("unexpected escape pattern in string")]
    UnexpectedEscapeInString,
    #[error("unexpected 'else' in match arm")]
    UnexpectedMatchElse,
    #[error("unexpected if condition in match arm")]
    UnexpectedMatchIf,
    #[error("unexpected meta key")]
    UnexpectedMetaKey,
    #[error("unexpected 'else' in switch arm")]
    UnexpectedSwitchElse,
    #[error("unexpected '?'")]
    UnexpectedNullCheck,
    #[error("unexpected token")]
    UnexpectedToken,
    #[error("unicode value out of range, the maximum is \\u{{10ffff}}")]
    UnicodeEscapeCodeOutOfRange,
    #[error("unterminated numeric escape code")]
    UnterminatedNumericEscapeCode,
    #[error("unterminated string")]
    UnterminatedString,
}

/// See [`ParserError`]
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
pub fn format_source_excerpt(source: &str, span: &Span, source_path: Option<&str>) -> String {
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
        let display_path = std::env::current_dir()
            .ok()
            .and_then(|dir| dir.to_str().and_then(|dir_str| path.strip_prefix(dir_str)))
            .unwrap_or(path);

        format!("{display_path} - {}:{}", start.line + 1, start.column + 1)
    } else {
        format!("{}:{}", start.line + 1, start.column + 1)
    };

    format!("{position_info}\n{padding}|\n{excerpt}")
}
