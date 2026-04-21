//! Handle-based wasm transport for dynamic Koto plugins.
//!
//! This module is an experimental transport sketch. It mirrors the logical ABI surface used by
//! native plugins, but replaces raw pointers and callback tables with integer handles and
//! flat-value structs suitable for wasm linear memory and imported/exported functions.
//!
//! The wasm transport is wired into `koto_runtime` as an experimental plugin host, while keeping
//! the native transport free to use direct pointers and callback tables.

use crate::{
    BinaryOp, IterableKind, UnaryOp,
    shared::{ABI_MAJOR_VERSION, ABI_MINOR_VERSION, KotoStatusCode},
};

type MutPtr = u32;
type ConstPtr = u32;
type BytePtr = u32;
type Size = u32;
type Word = u32;

const fn is_null_mut_ptr(ptr: u32) -> bool {
    ptr == 0
}

// Reuse the shared transport layout by binding its handle/size aliases in this module first.
#[expect(clippy::duplicate_mod)]
#[path = "transport_types.rs"]
mod transport_types;

pub use transport_types::*;

/// The current ABI major version for the wasm plugin transport.
pub const WASM_ABI_MAJOR_VERSION: u16 = ABI_MAJOR_VERSION;

/// The current ABI minor version for the wasm plugin transport.
pub const WASM_ABI_MINOR_VERSION: u16 = ABI_MINOR_VERSION;

/// The status returned by wasm ABI calls.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KotoStatus {
    /// The status code.
    pub code: KotoStatusCode,
    /// An optional owned runtime error handle.
    pub error: u32,
    /// True when the error represents an unimplemented operation.
    pub is_unimplemented: bool,
    /// An optional borrowed error message in wasm linear memory.
    pub message: KStringSlice,
}

impl KotoStatus {
    /// Returns a successful status.
    pub const fn ok() -> Self {
        Self {
            code: KotoStatusCode::Ok,
            error: 0,
            is_unimplemented: false,
            message: KStringSlice { ptr: 0, len: 0 },
        }
    }
}

impl Default for KotoStatus {
    fn default() -> Self {
        Self::ok()
    }
}

/// A function call context passed between host and wasm plugins.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CallContext {
    /// The call instance, or null if the function wasn't called as a method.
    pub instance: KValue,
    /// A linear-memory pointer to a contiguous array of [`KValue`] entries.
    pub args_ptr: u32,
    /// The number of arguments in `args_ptr`.
    pub arg_count: u32,
}

/// A pointer to a [`KValue`] stored in wasm linear memory.
pub type KValuePtr = u32;

/// A pointer to a [`KotoStatus`] stored in wasm linear memory.
pub type KotoStatusPtr = u32;

/// A pointer to a [`CallContext`] stored in wasm linear memory.
pub type CallContextPtr = u32;

/// A pointer to a [`KStringSlice`] stored in wasm linear memory.
pub type KStringSlicePtr = u32;

/// A pointer to a [`KValueView`] stored in wasm linear memory.
pub type KValueViewPtr = u32;

/// A pointer to a [`KTuple`] stored in wasm linear memory.
pub type KTuplePtr = u32;

/// A pointer to a [`KList`] stored in wasm linear memory.
pub type KListPtr = u32;

/// A pointer to a [`KMap`] stored in wasm linear memory.
pub type KMapPtr = u32;

/// A pointer to a [`KObject`] stored in wasm linear memory.
pub type KObjectPtr = u32;

/// A pointer to a [`KValueSlice`] stored in wasm linear memory.
pub type KValueSlicePtr = u32;

/// A pointer to a [`KMapData`] stored in wasm linear memory.
pub type KMapDataPtr = u32;

/// A pointer to a [`KMapEntryView`] stored in wasm linear memory.
pub type KMapEntryViewPtr = u32;

/// A pointer to a [`MetaKey`] stored in wasm linear memory.
pub type MetaKeyPtr = u32;

/// A pointer to a contiguous array of [`KValue`] entries in wasm linear memory.
pub type KValueArrayPtr = u32;

/// A pointer to a contiguous array of [`KotoMapEntry`] entries in wasm linear memory.
pub type KotoMapEntryArrayPtr = u32;

/// A plugin function implemented by a wasm module using the flat low-level ABI.
pub type WasmPluginFunction = unsafe extern "C" fn(
    ctx_ptr: CallContextPtr,
    user_data: u32,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// The required initialization entrypoint exported by a wasm plugin module using the flat ABI.
pub type WasmPluginInitV1 = unsafe extern "C" fn(out_ptr: KValuePtr, status_ptr: KotoStatusPtr);

/// An exported wasm plugin allocator used by the host to reserve guest linear memory.
pub type WasmPluginAlloc = unsafe extern "C" fn(size: u32, align: u32) -> u32;

/// An exported wasm plugin deallocator used by the host to release guest linear memory.
pub type WasmPluginFree = unsafe extern "C" fn(ptr: u32, size: u32, align: u32);

/// The host import used to construct an `i64` Koto value in wasm plugins.
pub type WasmHostFnValueMakeI64 = unsafe extern "C" fn(value: i64, out_ptr: KValuePtr);

/// The host import used to construct a Koto string from a linear-memory slice.
pub type WasmHostFnStringMake = unsafe extern "C" fn(value_ptr: KStringSlicePtr, out_ptr: u32);

/// The host import used to view a Koto string as a linear-memory slice.
pub type WasmHostFnStringAsSlice = unsafe extern "C" fn(string_ptr: u32, out_ptr: KStringSlicePtr);

/// The host import used to free a borrowed string slice buffer.
pub type WasmHostFnStringSliceFree = unsafe extern "C" fn(string_ptr: KStringSlicePtr);

/// The host import used to clone a host-owned Koto value handle.
pub type WasmHostFnValueClone = unsafe extern "C" fn(value_ptr: KValuePtr, out_ptr: KValuePtr);

/// The host import used to free a host-owned Koto value handle.
pub type WasmHostFnValueFree = unsafe extern "C" fn(value_ptr: KValuePtr);

/// The host import used to clone a borrowed value view into an owned value handle.
pub type WasmHostFnValueViewClone =
    unsafe extern "C" fn(value_view_ptr: KValueViewPtr, out_ptr: KValuePtr);

/// The host import used to free a borrowed value view handle.
pub type WasmHostFnValueViewFree = unsafe extern "C" fn(value_view_ptr: KValueViewPtr);

/// The host import used to create a tuple from a contiguous value slice.
pub type WasmHostFnTupleMake =
    unsafe extern "C" fn(values_ptr: KValueArrayPtr, len: u32, out_ptr: KTuplePtr);

/// The host import used to get the length of a tuple.
pub type WasmHostFnTupleLen = unsafe extern "C" fn(tuple_ptr: KTuplePtr) -> u32;

/// The host import used to get a borrowed tuple data view.
pub type WasmHostFnTupleData = unsafe extern "C" fn(tuple_ptr: KTuplePtr, out_ptr: KValueSlicePtr);

/// The host import used to free a borrowed value-slice view.
pub type WasmHostFnValueSliceFree = unsafe extern "C" fn(slice_ptr: KValueSlicePtr);

/// The host import used to create a list from a contiguous value slice.
pub type WasmHostFnListMake =
    unsafe extern "C" fn(values_ptr: KValueArrayPtr, len: u32, out_ptr: KListPtr);

/// The host import used to get the length of a list.
pub type WasmHostFnListLen = unsafe extern "C" fn(list_ptr: KListPtr) -> u32;

/// The host import used to get a borrowed list data view.
pub type WasmHostFnListData = unsafe extern "C" fn(list_ptr: KListPtr, out_ptr: KValueSlicePtr);

/// The host import used to create a map from contiguous entries.
pub type WasmHostFnMapMake =
    unsafe extern "C" fn(entries_ptr: KotoMapEntryArrayPtr, len: u32, out_ptr: KMapPtr);

/// The host import used to create a new exports map in wasm plugins.
pub type WasmHostFnMapNewWithType =
    unsafe extern "C" fn(type_name_ptr: KStringSlicePtr, out_ptr: KMapPtr);

/// The host import used to get the length of a map.
pub type WasmHostFnMapLen = unsafe extern "C" fn(map_ptr: KMapPtr) -> u32;

/// The host import used to get a borrowed map data view.
pub type WasmHostFnMapData = unsafe extern "C" fn(map_ptr: KMapPtr, out_ptr: KMapDataPtr);

/// The host import used to free a borrowed map-data view.
pub type WasmHostFnMapDataFree = unsafe extern "C" fn(map_data_ptr: KMapDataPtr);

/// The host import used to read one map entry from a borrowed map data view.
pub type WasmHostFnMapDataGetEntry =
    unsafe extern "C" fn(map_data_ptr: KMapDataPtr, index: u32, out_ptr: KMapEntryViewPtr);

/// The host import used to insert a value into a wasm plugin's exports map.
pub type WasmHostFnMapInsertValue = unsafe extern "C" fn(
    map_ptr: KMapPtr,
    key_ptr: KStringSlicePtr,
    value_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// The host import used to insert a meta value into a wasm plugin's exports map.
pub type WasmHostFnMapInsertMetaValue = unsafe extern "C" fn(
    map_ptr: KMapPtr,
    key_ptr: MetaKeyPtr,
    value_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// The host import used to register an exported wasm function as a Koto native function.
pub type WasmHostFnNativeFunctionMake = unsafe extern "C" fn(
    symbol_name_ptr: KStringSlicePtr,
    user_data: u32,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// The host import used to register a plugin-owned object handle.
pub type WasmHostFnObjectMake = unsafe extern "C" fn(user_data: u32, out_ptr: KObjectPtr);

/// The host import used to register a plugin-owned iterator handle.
pub type WasmHostFnIteratorMake = unsafe extern "C" fn(user_data: u32, out_ptr: KValuePtr);

/// Returns a plugin-owned object's type string.
pub type WasmPluginObjectTypeString =
    unsafe extern "C" fn(user_data: u32, out_ptr: KStringSlicePtr, status_ptr: KotoStatusPtr);

/// Drops a plugin-owned object previously registered with the host.
pub type WasmPluginObjectDrop = unsafe extern "C" fn(user_data: u32);

/// Looks up a named value on a plugin-owned object.
pub type WasmPluginObjectNamedValue = unsafe extern "C" fn(
    user_data: u32,
    key_ptr: KStringSlicePtr,
    out_ptr: KValuePtr,
    out_found_ptr: u32,
    status_ptr: KotoStatusPtr,
);

/// Assigns a named value on a plugin-owned object.
pub type WasmPluginObjectNamedValueAssign = unsafe extern "C" fn(
    user_data: u32,
    key_ptr: KStringSlicePtr,
    value_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Produces a display value for a plugin-owned object.
pub type WasmPluginObjectDisplay =
    unsafe extern "C" fn(user_data: u32, out_ptr: KValuePtr, status_ptr: KotoStatusPtr);

/// Calls a plugin-owned object.
pub type WasmPluginObjectCall = unsafe extern "C" fn(
    ctx_ptr: CallContextPtr,
    user_data: u32,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Returns the size of a plugin-owned object, if any.
pub type WasmPluginObjectSize = unsafe extern "C" fn(
    user_data: u32,
    out_ptr: u32,
    out_has_size_ptr: u32,
    status_ptr: KotoStatusPtr,
);

/// Returns whether a plugin-owned object is callable.
pub type WasmPluginObjectIsCallable =
    unsafe extern "C" fn(user_data: u32, out_ptr: u32, status_ptr: KotoStatusPtr);

/// Returns whether a plugin-owned object is iterable.
pub type WasmPluginObjectIterableKind = unsafe extern "C" fn(user_data: u32) -> IterableKind;

/// Produces an iterator for a plugin-owned object.
pub type WasmPluginObjectMakeIterator =
    unsafe extern "C" fn(user_data: u32, out_ptr: KValuePtr, status_ptr: KotoStatusPtr);

/// Applies a unary op to a plugin-owned object.
pub type WasmPluginObjectUnaryOp = unsafe extern "C" fn(
    user_data: u32,
    op: UnaryOp,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Applies a binary op to a plugin-owned object.
pub type WasmPluginObjectBinaryOp = unsafe extern "C" fn(
    user_data: u32,
    op: BinaryOp,
    rhs_ptr: KValuePtr,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Indexes a plugin-owned object.
pub type WasmPluginObjectIndex = unsafe extern "C" fn(
    user_data: u32,
    index_ptr: KValuePtr,
    out_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Assigns to a plugin-owned object via indexing.
pub type WasmPluginObjectIndexAssign = unsafe extern "C" fn(
    user_data: u32,
    index_ptr: KValuePtr,
    value_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Applies an assign-style binary op to a plugin-owned object.
pub type WasmPluginObjectBinaryOpAssign = unsafe extern "C" fn(
    user_data: u32,
    op: BinaryOp,
    rhs_ptr: KValuePtr,
    status_ptr: KotoStatusPtr,
);

/// Returns the next value from a plugin-owned iterator.
pub type WasmPluginIteratorNext = unsafe extern "C" fn(
    user_data: u32,
    out_ptr: KValuePtr,
    out_has_value_ptr: u32,
    status_ptr: KotoStatusPtr,
);

/// Returns whether a plugin-owned iterator supports reverse iteration.
pub type WasmPluginIteratorIsBidirectional =
    unsafe extern "C" fn(user_data: u32, out_ptr: u32, status_ptr: KotoStatusPtr);

/// Produces a copy of a plugin-owned iterator.
pub type WasmPluginIteratorCopy =
    unsafe extern "C" fn(user_data: u32, out_ptr: KValuePtr, status_ptr: KotoStatusPtr);

/// Returns the next value from the back of a plugin-owned iterator.
pub type WasmPluginIteratorNextBack = unsafe extern "C" fn(
    user_data: u32,
    out_ptr: KValuePtr,
    out_has_value_ptr: u32,
    status_ptr: KotoStatusPtr,
);

/// Drops a plugin-owned iterator previously registered with the host.
pub type WasmPluginIteratorDrop = unsafe extern "C" fn(user_data: u32);

/// Drops a plugin-owned native function previously registered with the host.
pub type WasmPluginNativeFunctionDrop = unsafe extern "C" fn(user_data: u32);

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "koto")]
unsafe extern "C" {
    /// Constructs an `i64` Koto value.
    pub fn koto_value_make_i64(value: i64, out_ptr: KValuePtr);

    /// Constructs a Koto string from a linear-memory slice.
    pub fn koto_string_make(value_ptr: KStringSlicePtr, out_ptr: u32);

    /// Returns a borrowed slice of a host-owned Koto string.
    pub fn koto_string_as_slice(string_ptr: u32, out_ptr: KStringSlicePtr);

    /// Frees a borrowed string slice buffer returned by the host.
    pub fn koto_string_slice_free(string_ptr: KStringSlicePtr);

    /// Clones a host-owned Koto value handle.
    pub fn koto_value_clone(value_ptr: KValuePtr, out_ptr: KValuePtr);

    /// Frees a host-owned Koto value handle.
    pub fn koto_value_free(value_ptr: KValuePtr);

    /// Clones a borrowed value view into an owned value handle.
    pub fn koto_value_view_clone(value_view_ptr: KValueViewPtr, out_ptr: KValuePtr);

    /// Frees a borrowed value view handle.
    pub fn koto_value_view_free(value_view_ptr: KValueViewPtr);

    /// Creates a tuple from a contiguous array of values.
    pub fn koto_tuple_make(values_ptr: KValueArrayPtr, len: u32, out_ptr: KTuplePtr);

    /// Returns the length of a tuple.
    pub fn koto_tuple_len(tuple_ptr: KTuplePtr) -> u32;

    /// Returns a borrowed tuple data view.
    pub fn koto_tuple_data(tuple_ptr: KTuplePtr, out_ptr: KValueSlicePtr);

    /// Frees a borrowed value-slice view.
    pub fn koto_value_slice_free(slice_ptr: KValueSlicePtr);

    /// Creates a list from a contiguous array of values.
    pub fn koto_list_make(values_ptr: KValueArrayPtr, len: u32, out_ptr: KListPtr);

    /// Returns the length of a list.
    pub fn koto_list_len(list_ptr: KListPtr) -> u32;

    /// Returns a borrowed list data view.
    pub fn koto_list_data(list_ptr: KListPtr, out_ptr: KValueSlicePtr);

    /// Creates a map from contiguous entries.
    pub fn koto_map_make(entries_ptr: KotoMapEntryArrayPtr, len: u32, out_ptr: KMapPtr);

    /// Creates a new exports map with the given type name.
    pub fn koto_map_new_with_type(type_name_ptr: KStringSlicePtr, out_ptr: KMapPtr);

    /// Returns the length of a map.
    pub fn koto_map_len(map_ptr: KMapPtr) -> u32;

    /// Returns a borrowed map data view.
    pub fn koto_map_data(map_ptr: KMapPtr, out_ptr: KMapDataPtr);

    /// Frees a borrowed map data view.
    pub fn koto_map_data_free(map_data_ptr: KMapDataPtr);

    /// Returns one entry from a borrowed map data view.
    pub fn koto_map_data_get_entry(
        map_data_ptr: KMapDataPtr,
        index: u32,
        out_ptr: KMapEntryViewPtr,
    );

    /// Inserts a named value into a plugin exports map.
    pub fn koto_map_insert_value(
        map_ptr: KMapPtr,
        key_ptr: KStringSlicePtr,
        value_ptr: KValuePtr,
        status_ptr: KotoStatusPtr,
    );

    /// Inserts a meta value into a plugin exports map.
    pub fn koto_map_insert_meta_value(
        map_ptr: KMapPtr,
        key_ptr: MetaKeyPtr,
        value_ptr: KValuePtr,
        status_ptr: KotoStatusPtr,
    );

    /// Registers a plugin-exported wasm symbol as a callable Koto native function.
    pub fn koto_native_function_make(
        symbol_name_ptr: KStringSlicePtr,
        user_data: u32,
        out_ptr: KValuePtr,
        status_ptr: KotoStatusPtr,
    );

    /// Registers a plugin-owned object handle.
    pub fn koto_object_make(user_data: u32, out_ptr: KObjectPtr);

    /// Registers a plugin-owned iterator handle.
    pub fn koto_iterator_make(user_data: u32, out_ptr: KValuePtr);
}

#[cfg(target_arch = "wasm32")]
fn ptr_to_u32<T>(ptr: *const T) -> u32 {
    ptr.cast::<u8>() as usize as u32
}

#[cfg(target_arch = "wasm32")]
fn mut_ptr_to_u32<T>(ptr: *mut T) -> u32 {
    ptr.cast::<u8>() as usize as u32
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs an `i64` Koto value.
pub unsafe fn value_make_i64(value: i64) -> KValue {
    let mut out = KValue::null();
    unsafe { koto_value_make_i64(value, mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs a string from a wasm linear-memory slice.
pub unsafe fn string_make(value: KStringSlice) -> KString {
    let mut out = KString::default();
    unsafe { koto_string_make(ptr_to_u32(&value), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that views a host string as a wasm linear-memory slice.
pub unsafe fn string_as_slice(string: KString) -> KStringSlice {
    let mut out = KStringSlice::default();
    unsafe { koto_string_as_slice(ptr_to_u32(&string), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that frees a borrowed string slice buffer.
pub unsafe fn string_slice_free(slice: KStringSlice) {
    unsafe { koto_string_slice_free(ptr_to_u32(&slice)) };
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that clones a host-owned Koto value handle.
pub unsafe fn value_clone(value: KValue) -> KValue {
    let mut out = KValue::null();
    unsafe { koto_value_clone(ptr_to_u32(&value), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that frees a host-owned Koto value handle.
pub unsafe fn value_free(value: KValue) {
    unsafe { koto_value_free(ptr_to_u32(&value)) };
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that clones a borrowed value view into an owned value handle.
pub unsafe fn value_view_clone(value: KValueView) -> KValue {
    let mut out = KValue::null();
    unsafe { koto_value_view_clone(ptr_to_u32(&value), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that frees a borrowed value view handle.
pub unsafe fn value_view_free(value: KValueView) {
    unsafe { koto_value_view_free(ptr_to_u32(&value)) };
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs a tuple from contiguous wasm values.
pub unsafe fn tuple_make(values: *const KValue, len: u32) -> KTuple {
    let mut out = KTuple::default();
    unsafe { koto_tuple_make(values as usize as u32, len, mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a tuple's length.
pub unsafe fn tuple_len(tuple: KTuple) -> u32 {
    unsafe { koto_tuple_len(ptr_to_u32(&tuple)) }
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a borrowed tuple data view.
pub unsafe fn tuple_data(tuple: KTuple) -> KValueSlice {
    let mut out = KValueSlice::default();
    unsafe { koto_tuple_data(ptr_to_u32(&tuple), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that frees a borrowed value-slice view.
pub unsafe fn value_slice_free(slice: KValueSlice) {
    unsafe { koto_value_slice_free(ptr_to_u32(&slice)) };
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs a list from contiguous wasm values.
pub unsafe fn list_make(values: *const KValue, len: u32) -> KList {
    let mut out = KList::default();
    unsafe { koto_list_make(values as usize as u32, len, mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a list's length.
pub unsafe fn list_len(list: KList) -> u32 {
    unsafe { koto_list_len(ptr_to_u32(&list)) }
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a borrowed list data view.
pub unsafe fn list_data(list: KList) -> KValueSlice {
    let mut out = KValueSlice::default();
    unsafe { koto_list_data(ptr_to_u32(&list), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs a map from contiguous entry values.
pub unsafe fn map_make(entries: *const KotoMapEntry, len: u32) -> KMap {
    let mut out = KMap::default();
    unsafe { koto_map_make(entries as usize as u32, len, mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that constructs a plugin export map with a given `@type`.
pub unsafe fn map_new_with_type(type_name: KStringSlice) -> KMap {
    let mut out = KMap::default();
    unsafe { koto_map_new_with_type(ptr_to_u32(&type_name), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a map's length.
pub unsafe fn map_len(map: KMap) -> u32 {
    unsafe { koto_map_len(ptr_to_u32(&map)) }
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns a borrowed map data view.
pub unsafe fn map_data(map: KMap) -> KMapData {
    let mut out = KMapData::default();
    unsafe { koto_map_data(ptr_to_u32(&map), mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that frees a borrowed map data view.
pub unsafe fn map_data_free(map_data: KMapData) {
    unsafe { koto_map_data_free(ptr_to_u32(&map_data)) };
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that returns one entry from a borrowed map data view.
pub unsafe fn map_data_get_entry(map_data: KMapData, index: u32) -> KMapEntryView {
    let mut out = KMapEntryView::default();
    unsafe { koto_map_data_get_entry(ptr_to_u32(&map_data), index, mut_ptr_to_u32(&mut out)) };
    out
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that inserts a named value into a plugin export map.
pub unsafe fn map_insert_value(map: KMap, key: KStringSlice, value: KValue) -> KotoStatus {
    let mut status = KotoStatus::default();
    unsafe {
        koto_map_insert_value(
            ptr_to_u32(&map),
            ptr_to_u32(&key),
            ptr_to_u32(&value),
            mut_ptr_to_u32(&mut status),
        );
    }
    status
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that inserts a meta value into a plugin export map.
pub unsafe fn map_insert_meta_value(map: KMap, key: MetaKey, value: KValue) -> KotoStatus {
    let mut status = KotoStatus::default();
    unsafe {
        koto_map_insert_meta_value(
            ptr_to_u32(&map),
            ptr_to_u32(&key),
            ptr_to_u32(&value),
            mut_ptr_to_u32(&mut status),
        );
    }
    status
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that registers a plugin-exported wasm symbol as a Koto native function.
pub unsafe fn native_function_make(
    symbol_name: KStringSlice,
    user_data: u32,
) -> (KotoStatus, KValue) {
    let mut status = KotoStatus::default();
    let mut out = KValue::null();
    unsafe {
        koto_native_function_make(
            ptr_to_u32(&symbol_name),
            user_data,
            mut_ptr_to_u32(&mut out),
            mut_ptr_to_u32(&mut status),
        );
    }
    (status, out)
}

#[cfg(target_arch = "wasm32")]
/// Calls the host import that registers a plugin-owned object handle.
pub unsafe fn object_make(user_data: u32) -> KObject {
    let mut out = KObject::default();
    unsafe { koto_object_make(user_data, mut_ptr_to_u32(&mut out)) };
    out
}

/// Registers a plugin-owned iterator handle and returns the resulting Koto value.
#[cfg(target_arch = "wasm32")]
pub unsafe fn iterator_make(user_data: u32) -> KValue {
    let mut out = KValue::null();
    // Safety: `out` points to guest linear memory owned by the current wasm instance.
    unsafe { koto_iterator_make(user_data, (&mut out as *mut KValue).cast::<u32>() as u32) };
    out
}

/// Indicates whether the wasm plugin host should treat a function as an import or export.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolDirection {
    /// A symbol exported by the plugin and called by the host.
    #[default]
    Export,
    /// A symbol imported from the host and called by the plugin.
    Import,
}

/// A symbolic wasm ABI entrypoint declaration.
///
/// This is descriptive metadata for the intended wasm transport shape. Unlike the native ABI,
/// wasm plugins are expected to exchange functions as flat imports/exports rather than through a
/// callback table.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Symbol {
    /// The symbol name in UTF-8 bytes within wasm linear memory.
    pub name: KStringSlice,
    /// Whether the symbol is imported or exported.
    pub direction: SymbolDirection,
}
