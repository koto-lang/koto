mod ast;
mod constant_pool;
mod error;
mod node;
pub mod num2;
pub mod num4;
mod parser;
mod parser2;
mod prec_climber;

pub use koto_lexer::{Position, Span};

pub use ast::*;
pub use constant_pool::ConstantPool;
pub use error::ParserError;
pub use node::*;
pub use parser::*;
pub use parser2::Parser as Parser2;

#[derive(Default)]
pub struct Options {
    pub export_all_top_level: bool,
}
