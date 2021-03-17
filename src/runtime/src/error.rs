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
    /// An error that occurred in the VM
    VmError { message: String },
    /// An error thrown in a Koto script
    KotoError {
        thrown_value: Value,
        // If the thrown value is a map, then its @display function will be evaluated in the VM.
        vm: Option<Arc<Mutex<Vm>>>,
    },
    /// An error that occurred in an external function
    ExternalError { message: String },
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

    pub fn make_koto_error(thrown_value: Value, vm: Vm) -> Self {
        Self::new(RuntimeErrorType::KotoError {
            thrown_value,
            vm: Some(Arc::new(Mutex::new(vm))),
        })
    }

    pub fn with_prefix(mut self, prefix: &str) -> Self {
        use RuntimeErrorType::*;

        self.error = match self.error {
            VmError { message } => VmError {
                message: format!("{}: {}", prefix, message),
            },
            ExternalError { message } => ExternalError {
                message: format!("{}: {}", prefix, message),
            },
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
            VmError { message } => message.clone(),
            ExternalError { message } => message.clone(),
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
macro_rules! make_vm_error {
    ($message:expr) => {{
        let error = $crate::RuntimeErrorType::VmError { message: $message };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        $crate::RuntimeError::new(error)
    }};
}

#[macro_export]
macro_rules! vm_error {
    ($error:expr) => {
        Err($crate::make_vm_error!(String::from($error)))
    };
    ($error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_vm_error!(format!($error, $($y),+)))
    };
}

#[macro_export]
macro_rules! make_external_error {
    ($message:expr) => {{
        let error = $crate::RuntimeErrorType::ExternalError { message: $message };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        $crate::RuntimeError::new(error)
    }};
}

#[macro_export]
macro_rules! external_error {
    ($error:expr) => {
        Err($crate::make_external_error!(String::from($error)))
    };
    ($error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_external_error!(format!($error, $($y),+)))
    };
}
