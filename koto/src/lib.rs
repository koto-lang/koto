#[macro_use]
extern crate pest_derive;

mod parser;
mod runtime;

pub use parser::KotoParser as Parser;
pub use runtime::{Error, Runtime, value::Value};
