mod call_stack;
mod external;
mod id;
mod runtime;
pub mod value;
mod value_iterator;
mod value_list;
mod value_map;
mod vm;

use koto_parser::LookupSlice;

use id::Id;
pub use runtime::Runtime;

pub use external::{ExternalFunction, ExternalValue};
pub use value::{make_external_value, type_as_string, RuntimeFunction, Value};
pub use value_list::{ValueList, ValueVec};
pub use value_map::{ValueHashMap, ValueMap};
pub use vm::Vm;

pub const EXTERNAL_DATA_ID: &str = "_external_data";

#[derive(Clone, Debug)]
pub enum Error {
    RuntimeError {
        message: String,
        start_pos: koto_parser::Position,
        end_pos: koto_parser::Position,
    },
    VmRuntimeError {
        message: String,
        instruction: usize,
    },
    ExternalError {
        message: String,
    },
}

pub type RuntimeResult = Result<Value, Error>;

#[macro_export]
macro_rules! make_runtime_error {
    ($node:expr, $message:expr) => {{
        let error = $crate::Error::RuntimeError {
            message: $message,
            start_pos: $node.start_pos,
            end_pos: $node.end_pos,
        };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        error
    }};
}

#[macro_export]
macro_rules! runtime_error {
    ($node:expr, $error:expr) => {
        Err($crate::make_runtime_error!($node, String::from($error)))
    };
    ($node:expr, $error:expr, $($y:expr),+) => {
        Err($crate::make_runtime_error!($node, format!($error, $($y),+)))
    };
}

#[macro_export]
macro_rules! make_vm_error {
    ($ip:expr, $message:expr) => {{
        let error = $crate::Error::VmRuntimeError {
            message: $message,
            instruction: $ip,
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
    ($ip:expr, $error:expr) => {
        Err($crate::make_vm_error!($ip, String::from($error)))
    };
    ($ip:expr, $error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_vm_error!($ip, format!($error, $($y),+)))
    };
}

#[macro_export]
macro_rules! make_external_error {
    ($message:expr) => {{
        let error = $crate::Error::ExternalError { message: $message };
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
    ($error:expr, $($y:expr),+) => {
        Err($crate::make_external_error!(format!($error, $($y),+)))
    };
}
