//! Contains the parser and AST format used by the Koto language

#![warn(missing_docs)]

mod ast;
mod constant_pool;
mod error;
mod node;
mod parser;

pub use crate::{
    ast::*,
    constant_pool::{Constant, ConstantIndex, ConstantPool},
    error::{format_error_with_excerpt, ParserError},
    node::*,
    parser::Parser,
};
pub use koto_lexer::{Position, Span};
