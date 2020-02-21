#[macro_use]
extern crate pest_derive;

mod parser;
mod runtime;

pub use parser::SongParser as Parser;
pub use runtime::{Error, Runtime};
