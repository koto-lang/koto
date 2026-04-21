//! Stable ABI definitions for dynamic Koto plugins

#![warn(missing_docs)]

mod shared;

pub mod native;
pub mod wasm;

pub use shared::*;
