//! Internal ABI selection for plugin authoring helpers.
//!
//! The plugin crate still targets the native callback-table transport today. Centralizing the
//! alias here makes the remaining wasm-specific work easier to isolate when the plugin-side
//! export/callback path is adapted to `koto_ffi::wasm`.

pub(crate) use koto_ffi::native::*;
