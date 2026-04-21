//! Helpers for bridging the native-shaped plugin wrappers onto the wasm transport.
//!
//! The current plugin implementation still stores the native transport types internally. The
//! wasm transport is instantiated from the same shared layout in `koto_ffi`, so selected values
//! can be converted with layout-preserving transmutes while the broader callback path is still
//! being adapted.

use crate::abi;
use koto_ffi::{KotoStatusCode, wasm};

use crate::Error;

pub(crate) fn string_slice(s: &str) -> wasm::KStringSlice {
    wasm::KStringSlice {
        ptr: s.as_ptr() as u32,
        len: s.len() as u32,
    }
}

pub(crate) fn status_to_error(status: wasm::KotoStatus) -> Error {
    if status.message.len == 0 {
        Error::new("wasm host operation failed")
    } else {
        Error::new("wasm host operation returned an error")
    }
}

pub(crate) fn error_status(message: &str) -> wasm::KotoStatus {
    wasm::KotoStatus {
        code: KotoStatusCode::Error,
        error: 0,
        is_unimplemented: false,
        message: string_slice(message),
    }
}

pub(crate) fn native_value_to_wasm(value: abi::KValue) -> wasm::KValue {
    // Safety: `koto_ffi::native::KValue` and `koto_ffi::wasm::KValue` are instantiated from the
    // same shared transport layout in `transport_types.rs`; only pointer/handle aliases differ.
    unsafe { std::mem::transmute::<abi::KValue, wasm::KValue>(value) }
}

pub(crate) fn wasm_value_to_native(value: wasm::KValue) -> abi::KValue {
    // Safety: see `native_value_to_wasm`.
    unsafe { std::mem::transmute::<wasm::KValue, abi::KValue>(value) }
}

pub(crate) fn wasm_map_to_native(handle: wasm::KMap) -> abi::KMap {
    // Safety: native and wasm `KMap` share the same transport layout.
    unsafe { std::mem::transmute::<wasm::KMap, abi::KMap>(handle) }
}

pub(crate) fn native_map_to_wasm(handle: abi::KMap) -> wasm::KMap {
    // Safety: native and wasm `KMap` share the same transport layout.
    unsafe { std::mem::transmute::<abi::KMap, wasm::KMap>(handle) }
}

pub(crate) fn wasm_string_to_native(handle: wasm::KString) -> abi::KString {
    // Safety: native and wasm `KString` share the same transport layout.
    unsafe { std::mem::transmute::<wasm::KString, abi::KString>(handle) }
}

pub(crate) fn native_string_to_wasm(handle: abi::KString) -> wasm::KString {
    unsafe { std::mem::transmute::<abi::KString, wasm::KString>(handle) }
}

pub(crate) fn native_meta_key_to_wasm(key: abi::MetaKey) -> wasm::MetaKey {
    // Safety: native and wasm `MetaKey` share the same transport layout.
    unsafe { std::mem::transmute::<abi::MetaKey, wasm::MetaKey>(key) }
}

pub(crate) fn wasm_tuple_to_native(handle: wasm::KTuple) -> abi::KTuple {
    unsafe { std::mem::transmute::<wasm::KTuple, abi::KTuple>(handle) }
}

pub(crate) fn native_tuple_to_wasm(handle: abi::KTuple) -> wasm::KTuple {
    unsafe { std::mem::transmute::<abi::KTuple, wasm::KTuple>(handle) }
}

pub(crate) fn wasm_list_to_native(handle: wasm::KList) -> abi::KList {
    unsafe { std::mem::transmute::<wasm::KList, abi::KList>(handle) }
}

pub(crate) fn native_list_to_wasm(handle: abi::KList) -> wasm::KList {
    unsafe { std::mem::transmute::<abi::KList, wasm::KList>(handle) }
}

pub(crate) fn wasm_object_to_native(handle: wasm::KObject) -> abi::KObject {
    unsafe { std::mem::transmute::<wasm::KObject, abi::KObject>(handle) }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn native_object_to_wasm(handle: abi::KObject) -> wasm::KObject {
    unsafe { std::mem::transmute::<abi::KObject, wasm::KObject>(handle) }
}

pub(crate) fn wasm_value_slice_to_native(slice: wasm::KValueSlice) -> abi::KValueSlice {
    unsafe { std::mem::transmute::<wasm::KValueSlice, abi::KValueSlice>(slice) }
}

pub(crate) fn native_value_slice_to_wasm(slice: abi::KValueSlice) -> wasm::KValueSlice {
    unsafe { std::mem::transmute::<abi::KValueSlice, wasm::KValueSlice>(slice) }
}

pub(crate) fn native_value_view_to_wasm(view: abi::KValueView) -> wasm::KValueView {
    unsafe { std::mem::transmute::<abi::KValueView, wasm::KValueView>(view) }
}

pub(crate) fn wasm_map_data_to_native(data: wasm::KMapData) -> abi::KMapData {
    unsafe { std::mem::transmute::<wasm::KMapData, abi::KMapData>(data) }
}

pub(crate) fn native_map_data_to_wasm(data: abi::KMapData) -> wasm::KMapData {
    unsafe { std::mem::transmute::<abi::KMapData, wasm::KMapData>(data) }
}
