//! Contains the lexer used by the Koto language

mod lexer;
mod span;

pub use lexer::{is_id_continue, is_id_start, KotoLexer as Lexer, Token};
pub use span::{Position, Span};
