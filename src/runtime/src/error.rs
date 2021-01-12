use {
    crate::Value,
    koto_bytecode::Chunk,
    std::{
        sync::Arc,
        {error, fmt},
    },
};

#[derive(Clone, Debug)]
pub enum RuntimeError {
    VmError {
        message: String,
        chunk: Arc<Chunk>,
        instruction: usize,
        extra_error: Option<Box<RuntimeError>>,
    },
    ExternalError {
        message: String,
    },
}

impl RuntimeError {
    pub fn with_prefix(self, prefix: &str) -> Self {
        use RuntimeError::*;

        match self {
            VmError {
                message,
                chunk,
                instruction,
                extra_error,
            } => VmError {
                message: format!("{}: {}", prefix, message),
                chunk,
                instruction,
                extra_error,
            },
            ExternalError { message } => ExternalError {
                message: format!("{}: {}", prefix, message),
            },
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RuntimeError::*;

        match &self {
            VmError {
                message,
                extra_error,
                ..
            } if extra_error.is_some() => {
                write!(f, "{}: {}", message, extra_error.as_ref().unwrap())
            }
            VmError { message, .. } => f.write_str(message),
            ExternalError { message } => f.write_str(message),
        }
    }
}

impl error::Error for RuntimeError {}

pub type RuntimeResult = Result<Value, RuntimeError>;

#[macro_export]
macro_rules! make_vm_error {
    ($chunk:expr, $ip:expr, $message:expr) => {{
        let error = $crate::RuntimeError::VmError {
            message: $message,
            chunk: $chunk,
            instruction: $ip,
            extra_error: None,
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
    ($chunk:expr, $ip:expr, $error:expr) => {
        Err($crate::make_vm_error!($chunk, $ip, String::from($error)))
    };
    ($chunk:expr, $ip:expr, $error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_vm_error!($chunk, $ip, format!($error, $($y),+)))
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
