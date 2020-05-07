mod ast;
mod constant_pool;
mod lookup;
mod node;
pub mod num2;
pub mod num4;
mod parser;
mod prec_climber;

pub use ast::*;
pub use constant_pool::ConstantPool;
pub use lookup::*;
pub use node::*;
pub use parser::*;

use std::fmt;

#[derive(Debug)]
pub enum ParserError {
    AstCapacityOverflow,
    PestSyntaxError(String),
    ParserError(String),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::ParserError::*;
        match self {
            AstCapacityOverflow => {
                f.write_str("There are more nodes in the program than the AST can support")
            }
            PestSyntaxError(error) => f.write_str(error),
            ParserError(error) => f.write_str(error),
        }
    }
}
