mod builtin_value;
mod call_stack;
mod runtime;
pub mod value;
mod value_iterator;
mod value_list;
mod value_map;

use koto_parser::{Id, LookupSlice};

pub use runtime::Runtime;

pub use builtin_value::BuiltinValue;
pub use value::Value;
pub use value_list::ValueList;
pub use value_map::ValueMap;

#[derive(Debug)]
pub enum Error {
    RuntimeError {
        message: String,
        start_pos: koto_parser::Position,
        end_pos: koto_parser::Position,
    },
    BuiltinError {
        message: String,
    },
}

pub type RuntimeResult<'a> = Result<Value<'a>, Error>;

#[macro_export]
macro_rules! make_runtime_error {
    ($node:expr, $message:expr) => {{
        let error = Error::RuntimeError {
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
        Err(crate::make_runtime_error!($node, String::from($error)))
    };
    ($node:expr, $error:expr, $($y:expr),+) => {
        Err(crate::make_runtime_error!($node, format!($error, $($y),+)))
    };
}
