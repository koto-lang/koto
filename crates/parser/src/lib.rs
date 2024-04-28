//! Contains the parser and AST format used by the Koto language

#![warn(missing_docs)]

mod ast;
mod constant_pool;
mod error;
mod node;
mod parser;
mod string_format_options;

pub use crate::{
    ast::*,
    constant_pool::{Constant, ConstantIndex, ConstantPool},
    error::{format_source_excerpt, Error, Result},
    node::*,
    parser::Parser,
    parser::TypeHint,
    string_format_options::{StringAlignment, StringFormatOptions},
};
pub use koto_lexer::{Position, RawStringDelimiter, Span, StringQuote, StringType};
