mod external;
mod id;
pub mod value;
mod value_iterator;
mod value_list;
mod value_map;
mod vm;

use id::Id;

pub use {
    external::{ExternalFunction, ExternalValue},
    value::{make_external_value, type_as_string, RuntimeFunction, Value},
    value_iterator::IntRange,
    value_list::{ValueList, ValueVec},
    value_map::{ValueHashMap, ValueMap},
    vm::Vm,
};

pub const EXTERNAL_DATA_ID: &str = "_external_data";

#[derive(Clone, Debug)]
pub enum Error {
    VmError { message: String, instruction: usize },
    ExternalError { message: String },
}

pub type RuntimeResult = Result<Value, Error>;

#[macro_export]
macro_rules! make_vm_error {
    ($ip:expr, $message:expr) => {{
        let error = $crate::Error::VmError {
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
    ($error:expr, $($y:expr),+ $(,)?) => {
        Err($crate::make_external_error!(format!($error, $($y),+)))
    };
}
