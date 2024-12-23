use thiserror::Error;

/// The different error types that can result from [Koto](crate::Koto) operations
#[derive(Debug, Error, Clone)]
#[allow(missing_docs)]
pub enum Error {
    #[error("{0}")]
    StringError(String),
    #[error("missing koto module in the prelude")]
    MissingPrelude,
    #[error("nothing to run")]
    NothingToRun,
    #[error("{error}")]
    CompileError {
        error: String,
        is_indentation_error: bool,
    },
}

impl Error {
    /// Returns true if the error was caused by the parser expecting indentation
    pub fn is_indentation_error(&self) -> bool {
        match self {
            Self::CompileError {
                is_indentation_error,
                ..
            } => *is_indentation_error,
            _ => false,
        }
    }
}

impl From<koto_runtime::Error> for Error {
    fn from(error: koto_runtime::Error) -> Self {
        use koto_runtime::ErrorKind as RuntimeError;

        // Runtime errors aren't Send+Sync when compiled without multi-threaded support,
        // so render the error message to a String.
        match error.error {
            RuntimeError::StringError(error) => Self::StringError(error),
            // Preserve compilation errors so they can be inspected by
            // [`is_indentation_error`](Self::is_indentation_error).
            RuntimeError::CompileError(error) => Self::from(error),
            _ => Self::StringError(error.to_string()),
        }
    }
}

impl From<koto_bytecode::LoaderError> for Error {
    fn from(error: koto_bytecode::LoaderError) -> Self {
        // Loader errors aren't Send+Sync when compiled without multi-threaded support,
        // so render the error message to a String.
        Self::CompileError {
            error: error.to_string(),
            is_indentation_error: error.is_indentation_error(),
        }
    }
}

/// The Result type returned by [Koto](crate::Koto) operations
pub type Result<T> = std::result::Result<T, Error>;
