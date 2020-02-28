mod builtins;
mod call_stack;
mod return_stack;
mod runtime;
mod value;
mod value_map;

pub use koto_parser::Ast;
pub use koto_parser::Id;
pub use koto_parser::KotoParser as Parser;
pub use koto_parser::LookupId;

pub use runtime::Runtime;
pub use value::Value;
pub use value_map::ValueMap;

#[derive(Debug)]
pub enum Error {
    RuntimeError {
        message: String,
        start_pos: koto_parser::Position,
        end_pos: koto_parser::Position,
    },
}

pub type RuntimeResult = Result<(), Error>;

#[macro_export]
macro_rules! make_runtime_error {
    ($node:expr, $message:expr) => {
        Error::RuntimeError {
            message: $message,
            start_pos: $node.start_pos,
            end_pos: $node.end_pos,
        }
    };
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
