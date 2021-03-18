use {
    crate::{UnaryOp, Value, Vm},
    koto_bytecode::Chunk,
    koto_parser::format_error_with_excerpt,
    parking_lot::Mutex,
    std::{
        sync::Arc,
        {error, fmt},
    },
};

#[derive(Clone, Debug)]
pub struct ErrorFrame {
    chunk: Arc<Chunk>,
    instruction: usize,
}

#[derive(Clone, Debug)]
pub enum RuntimeErrorType {
    /// A runtime error message
    StringError(String),
    /// An error thrown by a Koto script
    ///
    /// The value will either be a String or a Map.
    /// If the thrown value is a Map, then its @display function will be evaluated by the included
    /// VM when displaying the error.
    KotoError {
        thrown_value: Value,
        vm: Option<Arc<Mutex<Vm>>>,
    },
}

#[derive(Debug)]
pub struct RuntimeError {
    pub error: RuntimeErrorType,
    pub trace: Vec<ErrorFrame>,
}

impl RuntimeError {
    pub fn new(error: RuntimeErrorType) -> Self {
        Self {
            error,
            trace: Vec::new(),
        }
    }

    pub fn from_string(s: String) -> Self {
        Self::new(RuntimeErrorType::StringError(s))
    }

    pub fn from_koto_value(thrown_value: Value, vm: Vm) -> Self {
        Self::new(RuntimeErrorType::KotoError {
            thrown_value,
            vm: Some(Arc::new(Mutex::new(vm))),
        })
    }

    pub fn with_prefix(mut self, prefix: &str) -> Self {
        use RuntimeErrorType::StringError;

        self.error = match self.error {
            StringError(message) => StringError(format!("{}: {}", prefix, message)),
            other => other,
        };

        self
    }

    pub fn extend_trace(&mut self, chunk: Arc<Chunk>, instruction: usize) {
        self.trace.push(ErrorFrame { chunk, instruction });
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
                    .lock()
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
                        "Runtime error at instruction {}: {}",
                        frame.instruction, message
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
        $crate::RuntimeError::from_string($message)
    }};
}

#[macro_export]
macro_rules! runtime_error {
    ($error:expr) => {
        Err($crate::make_runtime_error!(String::from($error)))
    };
    ($error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_runtime_error!(format!($error, $($y),+)))
    };
}
