mod error;
mod format;
mod options;
mod trivia;

pub use crate::{
    error::{Error, ErrorKind, Result},
    format::format,
    options::FormatOptions,
    trivia::Trivia,
};
