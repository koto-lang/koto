//! Contains the lexer used by the Koto language

#![warn(missing_docs)]

mod lexer;
mod span;

pub use crate::{
    lexer::{
        is_id_continue, is_id_start, KotoLexer as Lexer, LexedToken, RawStringDelimiter,
        StringQuote, StringType, Token,
    },
    span::{Position, Span},
};
