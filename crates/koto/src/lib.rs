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
//!     Ok(_) => match koto.run() {
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

mod koto;
pub mod prelude;

pub use koto_bytecode as bytecode;
pub use koto_parser as parser;
pub use koto_runtime as runtime;
pub use koto_runtime::{derive, Borrow, BorrowMut, Error, ErrorKind, Ptr, PtrMut, Result};

pub use crate::koto::{Koto, KotoSettings};
