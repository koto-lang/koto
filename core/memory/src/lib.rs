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

mod rc;

pub use rc::*;
