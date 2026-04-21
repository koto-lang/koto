mod codec;
mod handles;
mod imports;
mod objects;
mod runtime;
#[cfg(test)]
mod tests;

pub(crate) use runtime::{is_wasm_import, load_wasm_module, resolve_wasm_module_path};
