//! Memory management utilities for Koto
//!
//! Currently, only reference-counted pointers without cycle detection are implemented.
//! The intent is that this crate can be expanded in the future with implementations of
//! `Ptr` and `PtrMut` that offer alternative memory management strategies.
//!
//! Making custom GC types that support trait objects or other DSTs is currently only
//! possible with nightly Rust, while the stabilization of DST custom coercions is pending [^1].
//! Until then, GC implementations for Ptr/PtrMut could be introduced with a nightly-only feature.
//!
//! [^1] <https://github.com/rust-lang/rust/issues/18598>

#![warn(missing_docs)]

#[cfg(all(feature = "arc", feature = "rc"))]
compile_error!("A single memory management feature can be enabled at a time");

mod address;
pub use address::Address;

#[cfg(feature = "arc")]
mod arc;
#[cfg(feature = "arc")]
pub use crate::arc::*;

#[cfg(feature = "rc")]
mod rc;
#[cfg(feature = "rc")]
pub use crate::rc::*;
