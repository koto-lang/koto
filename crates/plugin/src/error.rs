use crate::{KValue, runtime_error};
use koto_ffi as abi;
use std::{ffi::CString, fmt, mem::ManuallyDrop, ptr};

/// The Result type used by the plugin helpers.
pub type Result<T> = std::result::Result<T, Error>;

#[doc(hidden)]
pub struct RuntimeErrorHandle {
    error: *mut std::ffi::c_void,
    clone_error: abi::StatusFnCloneError,
    free_error: abi::StatusFnFreeError,
    is_unimplemented: bool,
}

impl RuntimeErrorHandle {
    fn from_status(status: abi::KotoStatus) -> Option<Self> {
        if status.error.is_null() {
            return None;
        }

        Some(Self {
            error: status.error,
            clone_error: status
                .clone_error_fn()
                .expect("runtime error status is missing clone_error"),
            free_error: status
                .free_error_fn()
                .expect("runtime error status is missing free_error"),
            is_unimplemented: status.is_unimplemented,
        })
    }

    fn into_status(self) -> abi::KotoStatus {
        let this = ManuallyDrop::new(self);
        abi::KotoStatus {
            code: abi::KotoStatusCode::Error,
            error: this.error,
            clone_error: this.clone_error as *const std::ffi::c_void,
            free_error: this.free_error as *const std::ffi::c_void,
            is_unimplemented: this.is_unimplemented,
            message: ptr::null_mut(),
        }
    }
}

impl Clone for RuntimeErrorHandle {
    fn clone(&self) -> Self {
        Self {
            error: unsafe { (self.clone_error)(self.error) },
            clone_error: self.clone_error,
            free_error: self.free_error,
            is_unimplemented: self.is_unimplemented,
        }
    }
}

impl Drop for RuntimeErrorHandle {
    fn drop(&mut self) {
        if !self.error.is_null() {
            unsafe { (self.free_error)(self.error) };
        }
    }
}

impl fmt::Debug for RuntimeErrorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeErrorHandle")
            .field("error", &self.error)
            .field("is_unimplemented", &self.is_unimplemented)
            .finish()
    }
}

/// Errors returned by the plugin helpers.
#[derive(Clone, Debug)]
pub enum Error {
    /// A plugin-local error message.
    Message(String),
    /// A runtime-originated error preserved as an opaque handle.
    Runtime(RuntimeErrorHandle),
}

impl Error {
    /// Creates a new error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub(crate) fn from_status(status: abi::KotoStatus) -> Self {
        if let Some(runtime_error) = RuntimeErrorHandle::from_status(status) {
            if !status.message.is_null() {
                let _ = unsafe { CString::from_raw(status.message) };
            }
            Self::Runtime(runtime_error)
        } else if status.message.is_null() {
            Self::new("plugin operation failed")
        } else {
            let message = unsafe { CString::from_raw(status.message) };
            Self::new(message.to_string_lossy().into_owned())
        }
    }

    /// Converts the error into an ABI status.
    pub fn into_status(self) -> abi::KotoStatus {
        match self {
            Self::Message(message) => {
                let message = CString::new(message).unwrap_or_else(|_| {
                    CString::new("plugin error message contained interior null bytes").unwrap()
                });
                abi::KotoStatus {
                    code: abi::KotoStatusCode::Error,
                    error: ptr::null_mut(),
                    clone_error: ptr::null(),
                    free_error: ptr::null(),
                    is_unimplemented: false,
                    message: message.into_raw(),
                }
            }
            Self::Runtime(error) => error.into_status(),
        }
    }

    /// Returns true if the error was created from an unimplemented operation.
    pub fn is_unimplemented_error(&self) -> bool {
        match self {
            Self::Message(message) => message.contains("is unimplemented for"),
            Self::Runtime(error) => error.is_unimplemented,
        }
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::Runtime(_) => f.write_str("runtime error"),
        }
    }
}

/// Returns an unexpected arguments error.
pub fn unexpected_args<T>(expected: &str, args: &[KValue]) -> Result<T> {
    let provided = args
        .iter()
        .map(KValue::type_as_string)
        .collect::<Vec<_>>()
        .join(", ");
    runtime_error!(format!(
        "Unexpected arguments. Expected: {expected}. Provided: |{provided}|"
    ))
}

/// Returns an unexpected type error.
pub fn unexpected_type<T>(expected: &str, value: &KValue) -> Result<T> {
    runtime_error!("expected {expected}, found {}", value.type_as_string())
}

/// Returns an unexpected arguments error including the provided instance value.
pub fn unexpected_args_after_instance<T>(
    expected: &str,
    instance: &KValue,
    args: &[KValue],
) -> Result<T> {
    let mut provided = Vec::with_capacity(args.len() + 1);
    provided.push(instance.clone());
    provided.extend(args.iter().cloned());
    unexpected_args(expected, &provided)
}
