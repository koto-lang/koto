use derive_name::VariantName;
use koto_lexer::Span;
use koto_parser::Node;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// The different error types that can be encountered during formatting
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum ErrorKind {
    #[error("expected {expected}, found '{}'", unexpected.variant_name())]
    UnexpectedNode { expected: String, unexpected: Node },
    #[error("An error occurred during lexing")]
    TokenError,
    #[error(transparent)]
    ParserError(#[from] koto_parser::Error),
}

/// An error that can be produced during formatting
#[derive(Error, Clone, Debug)]
#[error("{error}")]
pub struct Error {
    /// The error itself
    pub error: ErrorKind,

    /// The span in the source where the error occurred
    pub span: Span,
}

impl Error {
    /// Initializes a parser error with the specific error type and its associated span
    pub fn new(error: ErrorKind, span: Span) -> Self {
        Self { error, span }
    }
}

impl From<koto_parser::Error> for Error {
    fn from(error: koto_parser::Error) -> Self {
        Self {
            span: error.span,
            error: error.into(),
        }
    }
}
