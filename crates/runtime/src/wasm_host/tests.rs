use super::{
    handles::{HandleTable, WasmHandle},
    runtime::{is_wasm_import, resolve_wasm_module_path},
};
use crate::KValue;
use std::path::Path;

#[test]
fn detects_wasm_imports() {
    assert!(is_wasm_import("wasm:/tmp/foo"));
    assert!(!is_wasm_import("native:/tmp/foo"));
    assert!(!is_wasm_import("foo"));
}

#[test]
fn resolves_wasm_module_paths_relative_to_the_source_file() {
    let script_path = Path::new("/tmp/scripts/example.koto");
    let resolved = resolve_wasm_module_path("wasm:sample", Some(script_path)).unwrap();
    assert_eq!(resolved, Path::new("/tmp/scripts/sample"));
}

#[test]
fn stale_handles_are_rejected_after_slot_reuse() {
    let mut table = HandleTable::default();
    let first = table.insert(WasmHandle::ValueView(KValue::Null));
    assert!(matches!(
        table.get(first),
        Some(WasmHandle::ValueView(KValue::Null))
    ));

    let removed = table.remove(first);
    assert!(matches!(removed, Some(WasmHandle::ValueView(KValue::Null))));
    assert!(table.get(first).is_none());

    let second = table.insert(WasmHandle::ValueView(KValue::Bool(true)));
    assert_ne!(first, second);
    assert!(table.get(first).is_none());
    assert!(matches!(
        table.get(second),
        Some(WasmHandle::ValueView(KValue::Bool(true)))
    ));
}
