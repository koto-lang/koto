#[macro_use]
extern crate pest_derive;

mod parser;
mod runtime;

pub use parser::SongParser;
pub use runtime::{Error, Runtime};
