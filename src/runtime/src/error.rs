use {
    crate::Value,
    koto_bytecode::Chunk,
    koto_parser::format_error_with_excerpt,
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
pub enum RuntimeError {
    VmError {
        message: String,
        trace: Vec<ErrorFrame>,
    },
    ExternalError {
        message: String,
    },
    FunctionNotFound {
        name: String,
    },
}

impl RuntimeError {
    pub fn with_prefix(self, prefix: &str) -> Self {
        use RuntimeError::*;

        match self {
            VmError { message, trace } => VmError {
                message: format!("{}: {}", prefix, message),
                trace,
            },
            ExternalError { message } => ExternalError {
                message: format!("{}: {}", prefix, message),
            },
            FunctionNotFound { .. } => unimplemented!(),
        }
    }

    pub fn extend_trace(&mut self, chunk: Arc<Chunk>, instruction: usize) {
        if let Self::VmError { trace, .. } = self {
            trace.push(ErrorFrame { chunk, instruction });
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RuntimeError::*;

        match &self {
            VmError { message, .. } if f.alternate() => f.write_str(message),
            VmError { message, trace } => {
                let mut first_frame = true;
                for frame in trace.iter() {
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
            ExternalError { message } => f.write_str(message),
            FunctionNotFound { name } => write!(f, "Function '{}' not found", name),
        }
    }
}

impl error::Error for RuntimeError {}

pub type RuntimeResult = Result<Value, RuntimeError>;

#[macro_export]
macro_rules! make_vm_error {
    ($message:expr) => {{
        let error = $crate::RuntimeError::VmError {
            message: $message,
            trace: Vec::new(),
        };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        error
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
        let error = $crate::RuntimeError::ExternalError { message: $message };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        error
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
