//! Contains the parser and AST format used by the Koto language

#![warn(missing_docs)]

mod ast;
mod constant_pool;
mod error;
mod node;
mod parser;
mod string;
mod string_format_options;
mod string_slice;

pub use crate::{
    ast::*,
    constant_pool::{Constant, ConstantIndex, ConstantPool},
    error::{format_source_excerpt, Error, Result},
    node::*,
    parser::Parser,
    string::KString,
    string_format_options::{StringAlignment, StringFormatOptions, StringFormatRepresentation},
    string_slice::StringSlice,
};
pub use koto_lexer::{Position, RawStringDelimiter, Span, StringQuote, StringType};
