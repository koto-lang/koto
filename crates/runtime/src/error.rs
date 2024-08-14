use crate::{prelude::*, Ptr};
use koto_bytecode::{Chunk, LoaderError};
use koto_parser::format_source_excerpt;
use std::{error, fmt, time::Duration};
use thiserror::Error;

/// The different error types that can be thrown by the Koto runtime
#[derive(Error, Clone)]
#[allow(missing_docs)]
pub enum ErrorKind {
    #[error("{0}")]
    StringError(String),
    /// An error thrown by a Koto script
    ///
    /// The value will either be a String, or a value that implements @display, in which case the
    /// @display function will be evaluated by the included VM when displaying the error.
    #[error("{}", display_thrown_value(thrown_value, vm))]
    KotoError {
        /// The thrown value
        thrown_value: KValue,
        /// A VM that should be used to format the thrown value
        vm: KotoVm,
    },
    #[error("Execution timed out (the limit of {} seconds was reached)", .0.as_secs_f64())]
    Timeout(Duration),
    #[error(
        "Unexpected arguments.\n  Expected: {expected}\n  Provided: |{}|",
        value_types_as_string(unexpected)
    )]
    UnexpectedArguments {
        expected: String,
        unexpected: Vec<KValue>,
    },
    #[error("Unexpected type - expected: '{expected}', found: '{}'", unexpected.type_as_string())]
    UnexpectedType {
        expected: String,
        unexpected: KValue,
    },
    #[error("Unable to perform operation '{op}' with '{}' and '{}'", lhs.type_as_string(), rhs.type_as_string())]
    InvalidBinaryOp {
        lhs: KValue,
        rhs: KValue,
        op: BinaryOp,
    },
    #[error(transparent)]
    CompileError(#[from] LoaderError),
    #[error("Empty call stack")]
    EmptyCallStack,
    #[error("Missing sequence builder")]
    MissingSequenceBuilder,
    #[error("Missing string builder")]
    MissingStringBuilder,
    #[error("An unexpected error occurred, please report this as a bug at https://github.com/koto-lang/koto/issues")]
    UnexpectedError,
}

fn display_thrown_value(value: &KValue, vm: &KotoVm) -> String {
    let mut display_context = DisplayContext::with_vm(vm);

    if value.display(&mut display_context).is_ok() {
        display_context.result()
    } else {
        "Unable to display error message".into()
    }
}

impl fmt::Debug for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// An error thrown by the Koto runtime
#[derive(Clone, Debug)]
pub struct Error {
    /// The error that was thrown
    pub error: ErrorKind,
    /// The stack trace at the point when the error was thrown
    pub trace: Vec<ErrorFrame>,
}

impl Error {
    /// Initializes an error with the given internal error type
    pub(crate) fn new(error: ErrorKind) -> Self {
        Self {
            error,
            trace: Vec::new(),
        }
    }

    /// Initializes an error from a thrown Koto value
    pub(crate) fn from_koto_value(thrown_value: KValue, vm: KotoVm) -> Self {
        Self::new(ErrorKind::KotoError { thrown_value, vm })
    }

    /// Extends the error stack with the given [Chunk] and ip
    pub(crate) fn extend_trace(&mut self, chunk: Ptr<Chunk>, instruction: u32) {
        self.trace.push(ErrorFrame { chunk, instruction });
    }

    /// Modifies string errors to include the given prefix
    #[must_use]
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        use ErrorKind::StringError;

        self.error = match self.error {
            StringError(message) => StringError(format!("{prefix}: {message}")),
            other => other,
        };

        self
    }

    /// Returns true if the error was caused by the parser expecting indentation
    pub fn is_indentation_error(&self) -> bool {
        match &self.error {
            ErrorKind::CompileError(error) => error.is_indentation_error(),
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;

        for ErrorFrame { chunk, instruction } in self.trace.iter() {
            write!(f, "\n--- ")?;

            match chunk.debug_info.get_source_span(*instruction) {
                Some(span) => f.write_str(&format_source_excerpt(
                    &chunk.debug_info.source,
                    &span,
                    chunk.source_path.as_deref(),
                ))?,
                None => write!(f, "Runtime error at instruction {}", instruction)?,
            }
        }

        Ok(())
    }
}

impl error::Error for Error {}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Self::new(ErrorKind::StringError(error))
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Self::new(ErrorKind::StringError(error.into()))
    }
}

impl<T> From<T> for Error
where
    T: Into<ErrorKind>,
{
    fn from(error: T) -> Self {
        Self::new(error.into())
    }
}

/// A chunk and ip in a call stack where an error was thrown
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct ErrorFrame {
    pub chunk: Ptr<Chunk>,
    pub instruction: u32,
}

/// The Result type used by the Koto Runtime
pub type Result<T> = std::result::Result<T, Error>;

/// Creates a [crate::Error] from a message (with format-like behaviour), wrapped in `Err`
///
/// Wrapping the result in `Err` is a convenience for functions that need to return immediately when
/// an error has occured.
#[macro_export]
macro_rules! runtime_error {
    ($error:literal) => {
        Err($crate::Error::from(format!($error)))
    };
    ($error:expr) => {
        Err($crate::Error::from($error))
    };
    ($error:literal, $($y:expr),+ $(,)?) => {
        Err($crate::Error::from(format!($error, $($y),+)))
    };
}

/// Creates an error that describes a type mismatch
pub fn unexpected_type<T>(expected_str: &str, unexpected: &KValue) -> Result<T> {
    runtime_error!(ErrorKind::UnexpectedType {
        expected: expected_str.into(),
        unexpected: unexpected.clone(),
    })
}

/// Creates an unexpected arguments error containg the provided arguments
pub fn unexpected_args<T>(expected_str: &str, arguments: &[KValue]) -> Result<T> {
    runtime_error!(ErrorKind::UnexpectedArguments {
        expected: expected_str.into(),
        unexpected: arguments.into(),
    })
}

/// Creates an unexpected arguments error containing the provided instance and arguments
pub fn unexpected_args_after_instance<T>(
    expected_str: &str,
    instance: &KValue,
    arguments: &[KValue],
) -> Result<T> {
    let unexpected = std::iter::once(instance.clone())
        .chain(arguments.iter().cloned())
        .collect();
    runtime_error!(ErrorKind::UnexpectedArguments {
        expected: expected_str.into(),
        unexpected,
    })
}

fn value_types_as_string(values: &[KValue]) -> String {
    match values {
        [] => "".to_string(),
        [single_value] => single_value.type_as_string().to_string(),
        _ => {
            let mut result = String::new();
            let mut first = true;
            for value in values {
                if !first {
                    result.push_str(", ");
                }
                first = false;
                result.push_str(&value.type_as_string());
            }
            result
        }
    }
}
