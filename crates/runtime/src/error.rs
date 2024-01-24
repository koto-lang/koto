use crate::prelude::*;
use koto_bytecode::Chunk;
use koto_parser::format_source_excerpt;
use std::{error, fmt};
use thiserror::Error;

/// The different error types that can be thrown by the Koto runtime
#[derive(Error, Clone)]
#[allow(missing_docs)]
pub(crate) enum ErrorKind {
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
        vm: Vm,
    },
    #[error("Expected {expected}, but found {}", get_value_types(unexpected))]
    UnexpectedType {
        expected: String,
        unexpected: Vec<KValue>,
    },
    #[error("Unable to perform operation '{op}' with '{}' and '{}'", lhs.type_as_string(), rhs.type_as_string())]
    InvalidBinaryOp {
        lhs: KValue,
        rhs: KValue,
        op: BinaryOp,
    },
    #[error("Empty call stack")]
    EmptyCallStack,
    #[error("Missing sequence builder")]
    MissingSequenceBuilder,
    #[error("Missing string builder")]
    MissingStringBuilder,
}

fn display_thrown_value(value: &KValue, vm: &Vm) -> String {
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
    pub(crate) error: ErrorKind,
    pub(crate) trace: Vec<ErrorFrame>,
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
    pub(crate) fn from_koto_value(thrown_value: KValue, vm: Vm) -> Self {
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
                    &chunk.source_path,
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

impl From<ErrorKind> for Error {
    fn from(error: ErrorKind) -> Self {
        Self::new(error)
    }
}

/// A chunk and ip in a call stack where an error was thrown
///
/// See [RuntimeErrorTrace]
#[derive(Clone, Debug)]
pub struct ErrorFrame {
    chunk: Ptr<Chunk>,
    instruction: u32,
}

/// The Result type used by the Koto Runtime
pub type Result<T> = std::result::Result<T, Error>;

/// Creates a [crate::Error] from a message (with format-like behaviour), wrapped in `Err`
///
/// Wrapping the result in `Err` is a convenience for functions that need to return immediately when
/// an error has occured. See `make_runtime_error` for the internal function that creates the
/// internal error itself.
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
pub fn type_error<T>(expected_str: &str, unexpected: &KValue) -> Result<T> {
    type_error_with_slice(expected_str, &[unexpected.clone()])
}

/// Creates an error that describes a type mismatch with a slice of [KValue]s
pub fn type_error_with_slice<T>(expected_str: &str, unexpected: &[KValue]) -> Result<T> {
    runtime_error!(ErrorKind::UnexpectedType {
        expected: expected_str.into(),
        unexpected: unexpected.into(),
    })
}

fn get_value_types(values: &[KValue]) -> String {
    match values {
        [] => "no args".to_string(),
        [single_value] => single_value.type_as_string().to_string(),
        _ => {
            let mut types = String::from('(');
            let mut first = true;
            for value in values {
                if !first {
                    types.push_str(", ");
                }
                first = false;
                types.push_str(&value.type_as_string());
            }
            types.push(')');
            types
        }
    }
}
