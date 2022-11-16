use {
    crate::{UnaryOp, Value, Vm},
    koto_bytecode::Chunk,
    koto_parser::format_error_with_excerpt,
    std::{cell::RefCell, error, fmt, rc::Rc},
};

/// A chunk and ip in a call stack where an error was thrown
#[derive(Clone, Debug)]
pub struct ErrorFrame {
    chunk: Rc<Chunk>,
    instruction: usize,
}

/// The different error types that can be thrown by the Koto runtime
#[derive(Clone, Debug)]
pub(crate) enum RuntimeErrorType {
    /// A runtime error message
    StringError(String),
    /// An error thrown by a Koto script
    ///
    /// The value will either be a String or a Map.
    /// If the thrown value is a Map, then its @display function will be evaluated by the included
    /// VM when displaying the error.
    KotoError {
        /// The thrown value
        thrown_value: Value,
        /// A VM that should be used to format the thrown value
        vm: Option<Rc<RefCell<Vm>>>,
    },
}

/// An error thrown by the Koto runtime
#[derive(Clone, Debug)]
pub struct RuntimeError {
    pub(crate) error: RuntimeErrorType,
    pub(crate) trace: Vec<ErrorFrame>,
}

impl RuntimeError {
    /// Initializes an error with the given internal error type
    pub(crate) fn new(error: RuntimeErrorType) -> Self {
        Self {
            error,
            trace: Vec::new(),
        }
    }

    /// Initializes an error from a thrown Koto value
    pub(crate) fn from_koto_value(thrown_value: Value, vm: Vm) -> Self {
        Self::new(RuntimeErrorType::KotoError {
            thrown_value,
            vm: Some(Rc::new(RefCell::new(vm))),
        })
    }

    /// Extends the error stack with the given [Chunk] and ip
    pub(crate) fn extend_trace(&mut self, chunk: Rc<Chunk>, instruction: usize) {
        self.trace.push(ErrorFrame { chunk, instruction });
    }

    /// Modifies string errors to include the given prefix
    #[must_use]
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        use RuntimeErrorType::StringError;

        self.error = match self.error {
            StringError(message) => StringError(format!("{}: {}", prefix, message)),
            other => other,
        };

        self
    }
}

impl From<String> for RuntimeError {
    fn from(error: String) -> Self {
        Self::new(RuntimeErrorType::StringError(error))
    }
}

impl From<&str> for RuntimeError {
    fn from(error: &str) -> Self {
        Self::new(RuntimeErrorType::StringError(error.into()))
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use {RuntimeErrorType::*, Value::*};

        let message = match &self.error {
            StringError(s) => s.clone(),
            KotoError { thrown_value, vm } => match (&thrown_value, vm) {
                (Str(message), _) => message.to_string(),
                (Map(_), Some(vm)) => match vm
                    .borrow_mut()
                    .run_unary_op(UnaryOp::Display, thrown_value.clone())
                {
                    Ok(Str(message)) => message.to_string(),
                    Ok(other) => format!(
                        "Error while getting error message, expected string, found '{}'",
                        other.type_as_string()
                    ),
                    Err(_) => "Unable to get error message".to_string(),
                },
                _ => "Unable to get error message".to_string(),
            },
        };

        if f.alternate() {
            f.write_str(&message)
        } else {
            let mut first_frame = true;
            for frame in self.trace.iter() {
                let frame_message = if first_frame {
                    first_frame = false;
                    Some(message.as_str())
                } else {
                    None
                };

                match frame.chunk.debug_info.get_source_span(frame.instruction) {
                    Some(span) => f.write_str(&format_error_with_excerpt(
                        frame_message,
                        &frame.chunk.source_path,
                        &frame.chunk.debug_info.source,
                        span.start,
                        span.end,
                    ))?,
                    None => write!(
                        f,
                        "Runtime error at instruction {}: {message}",
                        frame.instruction,
                    )?,
                };
            }
            Ok(())
        }
    }
}

impl error::Error for RuntimeError {}

pub type RuntimeResult = Result<Value, RuntimeError>;

#[macro_export]
macro_rules! make_runtime_error {
    ($message:expr) => {{
        #[cfg(panic_on_runtime_error)]
        {
            panic!($message);
        }
        $crate::RuntimeError::from($message)
    }};
}

#[macro_export]
macro_rules! runtime_error {
    ($error:literal) => {
        Err($crate::make_runtime_error!(format!($error)))
    };
    ($error:expr) => {
        Err($crate::make_runtime_error!($error))
    };
    ($error:literal, $($y:expr),+ $(,)?) => {
        Err($crate::make_runtime_error!(format!($error, $($y),+)))
    };
}

pub fn type_error<T>(
    prefix: &str,
    expected_str: &str,
    unexpected: &Value,
) -> Result<T, RuntimeError> {
    let error = make_runtime_error!(format!(
        "Expected {expected_str}, but found {}.",
        unexpected.type_as_string()
    ));

    if prefix.is_empty() {
        Err(error)
    } else {
        Err(error.with_prefix(prefix))
    }
}

pub fn type_error_with_slice<T>(
    prefix: &str,
    expected_str: &str,
    unexpected: &[Value],
) -> Result<T, RuntimeError> {
    let message = match unexpected {
        [] => "no args".to_string(),
        [single_arg] => single_arg.type_as_string().to_string(),
        _ => {
            let mut types = String::from('(');
            let mut first = true;
            for value in unexpected {
                if !first {
                    types.push_str(", ");
                }
                first = false;
                types.push_str(&value.type_as_string());
            }
            types.push(')');
            types
        }
    };
    let error = make_runtime_error!(format!("Expected {expected_str}, but found {message}."));
    if prefix.is_empty() {
        Err(error)
    } else {
        Err(error.with_prefix(prefix))
    }
}
