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
//!             Value::Number(n) => println!("{n}"), // 3.0
//!             other => panic!("Unexpected result: {}", other),
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

mod koto;
pub mod prelude;

pub use {
    crate::koto::{Koto, KotoError, KotoSettings},
    koto_bytecode as bytecode, koto_parser as parser, koto_runtime as runtime,
};
