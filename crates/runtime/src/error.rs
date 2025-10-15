use crate::{Ptr, prelude::*};
use koto_bytecode::{Chunk, ModuleLoaderError};
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
    #[error("{}", display_thrown_value(thrown_value, vm.as_deref()))]
    KotoError {
        /// The thrown value
        thrown_value: KValue,
        /// A VM that should be used to format the thrown value
        //
        // This is None by default, and initialized to Some when errors aren't caught within a
        // script and are being propagated outside of the runtime, see Vm::execute_instructions.
        vm: Option<Box<KotoVm>>,
    },
    #[error("execution timed out (the limit of {} seconds was reached)", .0.as_secs_f64())]
    Timeout(Duration),
    #[error("unable to borrow an object that is already mutably borrowed")]
    UnableToBorrowObject,
    #[error(
        "Unexpected arguments.\n  Expected: {expected}\n  Provided: |{}|",
        value_types_as_string(unexpected)
    )]
    UnexpectedArguments {
        expected: String,
        unexpected: Vec<KValue>,
    },
    #[error("insufficient arguments ({actual}, expected {expected})")]
    InsufficientArguments { expected: u8, actual: u8 },
    #[error("too many arguments ({actual}, expected {expected})")]
    TooManyArguments { expected: u8, actual: u8 },
    #[error("expected {expected}, found {}", unexpected.type_as_string())]
    UnexpectedType {
        expected: String,
        unexpected: KValue,
    },
    #[error("expected {expected}, found {unexpected}")]
    UnexpectedObjectType {
        expected: &'static str,
        unexpected: KString,
    },
    #[error("{fn_name} is unimplemented for {object_type}")]
    Unimplemented {
        fn_name: &'static str,
        object_type: KString,
    },
    #[error("access assignment is not implemented for this object")]
    UnimplementedAccessAssign,
    #[error("unable to perform operation '{op}' with '{}' and '{}'", lhs.type_as_string(), rhs.type_as_string())]
    InvalidBinaryOp {
        lhs: KValue,
        rhs: KValue,
        op: BinaryOp,
    },

    #[error("empty call stack")]
    EmptyCallStack,
    #[error("missing sequence builder")]
    MissingSequenceBuilder,
    #[error("missing string builder")]
    MissingStringBuilder,
    #[error("this operation is unsupported on this platform")]
    UnsupportedPlatform,
    #[error(
        "an unexpected error occurred, please report this as a bug at\nhttps://github.com/koto-lang/koto/issues"
    )]
    UnexpectedError,

    #[error(transparent)]
    CompileError(#[from] ModuleLoaderError),
}

fn display_thrown_value(value: &KValue, vm: Option<&KotoVm>) -> String {
    if let Some(vm) = vm {
        let mut display_context = DisplayContext::with_vm(vm);

        if value.display(&mut display_context).is_ok() {
            return display_context.result();
        }
    }

    "Unable to display error message".into()
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
    /// Additional context attached to the error
    pub context: Vec<String>,
    /// The stack trace at the point when the error was thrown
    pub trace: Vec<InstructionFrame>,
}

impl Error {
    /// Initializes an error with the given [`ErrorKind`]
    pub fn new(error: ErrorKind) -> Self {
        Self {
            error,
            context: Vec::new(),
            trace: Vec::new(),
        }
    }

    /// Initializes an error with the given [`ErrorKind`] and the top frame of the stack trace
    ///
    /// This is useful for errors thrown from native functions after they were initially called
    /// from the runtime, e.g. iterator adaptors that can throw errors when the iterator is being
    /// lazily consumed; it's helpful to highlight the adaptor itself at the top of the stack trace.
    pub fn with_error_frame(error: ErrorKind, error_frame: InstructionFrame) -> Self {
        Self {
            error,
            context: Vec::new(),
            trace: vec![error_frame],
        }
    }

    /// Adds additional context to the error
    #[must_use]
    pub fn with_context(mut self, prefix: String) -> Self {
        self.context.push(prefix);
        self
    }

    /// Returns true if the error was caused by the parser expecting indentation
    pub fn is_indentation_error(&self) -> bool {
        match &self.error {
            ErrorKind::CompileError(error) => error.is_indentation_error(),
            _ => false,
        }
    }

    /// Returns true if the error kind is [`ErrorKind::Unimplemented`]
    pub fn is_unimplemented_error(&self) -> bool {
        matches!(&self.error, ErrorKind::Unimplemented { .. })
    }

    /// Initializes an error from a thrown Koto value
    pub(crate) fn from_koto_value(thrown_value: KValue) -> Self {
        Self::new(ErrorKind::KotoError {
            thrown_value,
            vm: None, // A vm will be spawned if the error propagates outside of the runtime
        })
    }

    /// Extends the error stack with the given [Chunk] and instruction pointer
    pub(crate) fn extend_trace(&mut self, instruction_frame: InstructionFrame) {
        self.trace.push(instruction_frame);
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        for context in self.context.iter() {
            write!(f, " ({context})")?;
        }

        for InstructionFrame { chunk, instruction } in self.trace.iter() {
            write!(f, "\n--- ")?;

            if let Some(span) = chunk.debug_info.get_source_span(*instruction) {
                f.write_str(&format_source_excerpt(
                    &chunk.debug_info.source,
                    &span,
                    chunk.path.as_deref(),
                ))?;

                continue;
            }

            write!(
                f,
                "Runtime error at instruction {} in chunk {:p}",
                instruction, &chunk
            )?
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

/// A chunk and instruction pointer in a call stack where an error was thrown
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct InstructionFrame {
    pub chunk: Ptr<Chunk>,
    pub instruction: u32,
}

/// The Result type used by the Koto Runtime
pub type Result<T> = std::result::Result<T, Error>;

/// Creates a [crate::Error] from a message (with format-like behaviour), wrapped in `Err`
///
/// Wrapping the result in `Err` is a convenience for functions that need to return immediately when
/// an error has occurred.
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

/// Creates an unexpected arguments error containing the provided arguments
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
