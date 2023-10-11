use std::path::PathBuf;

use koto_bytecode::LoaderError;
use koto_runtime::RuntimeError;

use thiserror::Error;

/// The error type returned by [Koto](crate::Koto) operations
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CompileError(#[from] LoaderError),

    #[error(transparent)]
    RuntimeError(#[from] RuntimeError),

    #[error("Missing compiled chunk, call compile() before calling run()")]
    NothingToRun,

    #[error("The path '{0}' couldn't be found")]
    InvalidScriptPath(PathBuf),

    #[error("The koto module wasn't found in the runtime's prelude")]
    MissingKotoModuleInPrelude,

    #[error("Expected a Map for the exported 'tests', found '{0}'")]
    InvalidTestsType(String),

    #[error("Function not found")]
    FunctionNotFound,
}

impl Error {
    /// Returns true if the error is a complier 'expected indentation' error
    ///
    /// This is useful in the REPL, where an indentation error signals that the expression should be
    /// continued on an indented line.
    pub fn is_indentation_error(&self) -> bool {
        match &self {
            Self::CompileError(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

/// The Result type returned by [Koto](crate::Koto) operations
pub type Result<T> = std::result::Result<T, Error>;
