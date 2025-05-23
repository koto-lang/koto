//! # Koto
//!
//! Pulls together the compiler and runtime for the Koto programming language.
//!
//! Programs can be compiled and executed with the [Koto] struct.
//!
//! ## Example
//!
//! ```
//! use koto::prelude::*;
//!
//! let mut koto = Koto::default();
//! match koto.compile("1 + 2") {
//!     Ok(chunk) => match koto.run(chunk) {
//!         Ok(result) => match result {
//!             KValue::Number(n) => println!("{n}"), // 3.0
//!             other => panic!("Unexpected result type: {}", other.type_as_string()),
//!         },
//!         Err(runtime_error) => {
//!             panic!("Runtime error: {runtime_error}");
//!         }
//!     },
//!     Err(compiler_error) => {
//!         panic!("Compiler error: {compiler_error}");
//!     }
//! }
//! ```

#![warn(missing_docs)]

mod error;
mod koto;
pub mod prelude;

pub use koto_bytecode as bytecode;
pub use koto_parser as parser;
pub use koto_runtime as runtime;
pub use koto_runtime::{Borrow, BorrowMut, ErrorKind, Ptr, PtrMut, derive};

#[cfg(feature = "serde")]
pub use koto_serde as serde;

pub use crate::error::{Error, Result};
pub use crate::koto::{CompileArgs, Koto, KotoSettings};
