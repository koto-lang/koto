//! Pointer-based native transport for dynamic Koto plugins.

pub use crate::shared::*;

use std::{
    ffi::{c_char, c_void},
    mem::transmute,
};

type MutPtr = *mut c_void;
type ConstPtr = *const c_void;
type BytePtr = *const u8;
type Size = usize;
type Word = usize;

const fn is_null_mut_ptr(ptr: *mut c_void) -> bool {
    ptr.is_null()
}

// Reuse the shared transport layout by binding its pointer/size aliases in this module first.
#[path = "transport_types.rs"]
mod transport_types;

pub use transport_types::*;

/// Clones a runtime-owned error handle stored in [`KotoStatus`].
pub type StatusFnCloneError = unsafe extern "C" fn(error: *mut c_void) -> *mut c_void;

/// Frees a runtime-owned error handle stored in [`KotoStatus`].
pub type StatusFnFreeError = unsafe extern "C" fn(error: *mut c_void);

/// The status returned by ABI calls.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KotoStatus {
    /// The status code.
    pub code: KotoStatusCode,
    /// An optional owned runtime error handle.
    ///
    /// Ownership is transferred to the receiver on non-OK results.
    pub error: *mut c_void,
    /// A nullable clone hook for [`Self::error`].
    pub clone_error: *const c_void,
    /// A nullable drop hook for [`Self::error`].
    pub free_error: *const c_void,
    /// True when the error represents an unimplemented operation.
    pub is_unimplemented: bool,
    /// An optional owned C string containing an error message.
    ///
    /// Ownership is transferred to the receiver on non-OK results.
    pub message: *mut c_char,
}

impl KotoStatus {
    /// Returns a successful status.
    pub const fn ok() -> Self {
        Self {
            code: KotoStatusCode::Ok,
            error: std::ptr::null_mut(),
            clone_error: std::ptr::null(),
            free_error: std::ptr::null(),
            is_unimplemented: false,
            message: std::ptr::null_mut(),
        }
    }

    /// Returns [`Self::clone_error`] as a function pointer, when present.
    pub fn clone_error_fn(self) -> Option<StatusFnCloneError> {
        if self.clone_error.is_null() {
            None
        } else {
            Some(unsafe { transmute::<*const c_void, StatusFnCloneError>(self.clone_error) })
        }
    }

    /// Returns [`Self::free_error`] as a function pointer, when present.
    pub fn free_error_fn(self) -> Option<StatusFnFreeError> {
        if self.free_error.is_null() {
            None
        } else {
            Some(unsafe { transmute::<*const c_void, StatusFnFreeError>(self.free_error) })
        }
    }
}

/// A function call context passed from the host to plugins.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CallContext {
    /// The call instance, or null if the function wasn't called as a method.
    pub instance: KValue,
    /// The function arguments.
    pub args: *const KValue,
    /// The number of arguments in `args`.
    pub arg_count: usize,
}

/// A function implemented by a plugin.
pub type KotoPluginFunction = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    ctx: CallContext,
    user_data: *mut c_void,
    out: *mut KValue,
) -> KotoStatus;

/// A callback used to drop plugin-owned userdata.
pub type KotoPluginDrop = unsafe extern "C" fn(user_data: *mut c_void);

/// Initializes runtime-owned plugin object data.
pub type ObjectFnInit = unsafe extern "C" fn(storage: *mut c_void, source: *mut c_void);

/// Drops runtime-owned plugin object data.
pub type ObjectFnDropData = unsafe extern "C" fn(storage: *mut c_void);

/// The runtime-owned storage description for a plugin object.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KotoObjectDataV1 {
    /// The size of this struct.
    pub struct_size: usize,
    /// The size of the object data in bytes.
    pub size: usize,
    /// The required alignment of the object data.
    pub align: usize,
    /// Initializes the allocated storage from the provided source pointer.
    pub init: ObjectFnInit,
    /// Drops the initialized object data in place.
    pub drop: ObjectFnDropData,
    /// An opaque source pointer consumed by `init`.
    pub source: *mut c_void,
}

/// Returns a plugin-owned object's type name.
pub type ObjectFnTypeString =
    unsafe extern "C" fn(host_api: *const KotoHostApiV1, object: KObject) -> KStringSlice;

/// Looks up a named value on a plugin-owned object.
pub type ObjectFnNamedValue = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    key: KStringSlice,
    out: *mut KValue,
    out_found: *mut bool,
) -> KotoStatus;

/// Assigns a named value on a plugin-owned object.
pub type ObjectFnNamedValueAssign = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    key: KStringSlice,
    value: KValue,
) -> KotoStatus;

/// Calls a plugin-owned callable object.
pub type ObjectFnCall = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    ctx: CallContext,
    out: *mut KValue,
) -> KotoStatus;

/// Produces a display value for a plugin-owned object.
pub type ObjectFnDisplay = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    out: *mut KValue,
) -> KotoStatus;

/// Returns a plugin-owned object's size, if any.
pub type ObjectFnSize = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    out: *mut usize,
    out_has_value: *mut bool,
) -> KotoStatus;

/// Returns whether the object behaves like a function.
pub type ObjectFnIsCallable = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    out: *mut bool,
) -> KotoStatus;

/// Indexes a plugin-owned object.
pub type ObjectFnIndex = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    index: KValue,
    out: *mut KValue,
) -> KotoStatus;

/// Assigns via indexing on a plugin-owned object.
pub type ObjectFnIndexAssign = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    index: KValue,
    value: KValue,
) -> KotoStatus;

/// Compares a plugin-owned object for equality.
pub type ObjectFnEqual = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    other: KValue,
    out: *mut bool,
) -> KotoStatus;

/// Runs a unary operation on a plugin-owned object.
pub type ObjectFnUnaryOp = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    op: UnaryOp,
    out: *mut KValue,
) -> KotoStatus;

/// Runs a binary operation on a plugin-owned object.
pub type ObjectFnBinaryOp = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    op: BinaryOp,
    rhs: KValue,
    out: *mut KValue,
) -> KotoStatus;

/// Runs an in-place binary operation on a plugin-owned object.
pub type ObjectFnBinaryOpAssign = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    op: BinaryOp,
    rhs: KValue,
) -> KotoStatus;

/// Returns a plugin-owned object's iterable kind.
pub type ObjectFnIterableKind =
    unsafe extern "C" fn(host_api: *const KotoHostApiV1, object: KObject) -> IterableKind;

/// Produces an iterator value for a plugin-owned iterable object.
pub type ObjectFnMakeIterator = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    out: *mut KValue,
) -> KotoStatus;

/// Returns the next value from a plugin-owned iterator.
pub type ObjectFnIteratorNext = unsafe extern "C" fn(
    host_api: *const KotoHostApiV1,
    object: KObject,
    out: *mut KValue,
    out_has_value: *mut bool,
) -> KotoStatus;

/// The v1 callback table used for plugin-owned objects.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KotoPluginObjectV1 {
    /// The size of this struct.
    pub struct_size: usize,
    /// A per-type tag used for downcasting host-backed Rust objects.
    pub type_tag: usize,
    /// Returns the object's type name.
    pub type_string: ObjectFnTypeString,
    /// Looks up a named value on the object.
    pub named_value: ObjectFnNamedValue,
    /// Returns whether the object is iterable.
    pub iterable_kind: ObjectFnIterableKind,
    /// Returns the next value from the object if it's a forward iterator.
    pub iterator_next: ObjectFnIteratorNext,
    /// Returns the next value from the end if it's a bidirectional iterator.
    pub iterator_next_back: ObjectFnIteratorNext,
    /// Assigns a named value on the object.
    pub named_value_assign: ObjectFnNamedValueAssign,
    /// Calls the object when it behaves like a function.
    pub call: ObjectFnCall,
    /// Produces a display value for the object.
    pub display: ObjectFnDisplay,
    /// Returns the object's size, if any.
    pub size: ObjectFnSize,
    /// Returns whether the object behaves like a function.
    pub is_callable: ObjectFnIsCallable,
    /// Indexes the object.
    pub index: ObjectFnIndex,
    /// Assigns via indexing on the object.
    pub index_assign: ObjectFnIndexAssign,
    /// Compares the object for equality.
    pub equal: ObjectFnEqual,
    /// Runs unary operators like `@negate`.
    pub unary_op: ObjectFnUnaryOp,
    /// Runs binary operators like `@+`, `@r+`, `@-`, and `@/`.
    pub binary_op: ObjectFnBinaryOp,
    /// Runs in-place binary operators like `@+=`.
    pub binary_op_assign: ObjectFnBinaryOpAssign,
    /// Produces an iterator value for iterable objects.
    pub make_iterator: ObjectFnMakeIterator,
}

/// The entrypoint used to initialize a dynamic Koto plugin.
pub type KotoPluginInitV1 =
    unsafe extern "C" fn(host_api: *const KotoHostApiV1, out: *mut KValue) -> KotoStatus;

/// The v1 host API exposed to dynamic Koto plugins.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub struct KotoHostApiV1 {
    /// The ABI major version.
    pub abi_major: u16,
    /// The ABI minor version.
    pub abi_minor: u16,
    /// The size of this struct.
    pub struct_size: usize,
    pub map_new_with_type: unsafe extern "C" fn(type_name: KStringSlice) -> KMap,
    pub map_insert_value: unsafe extern "C" fn(map: KMap, key: KStringSlice, value: KValue),
    pub map_insert_meta_value: unsafe extern "C" fn(map: KMap, key: MetaKey, value: KValue),
    pub native_function_make: unsafe extern "C" fn(
        function: KotoPluginFunction,
        user_data: *mut c_void,
        drop_user_data: KotoPluginDrop,
    ) -> OpaqueHandle,
    pub value_make_null: unsafe extern "C" fn() -> KValue,
    pub value_make_bool: unsafe extern "C" fn(value: bool) -> KValue,
    pub value_make_i64: unsafe extern "C" fn(value: i64) -> KValue,
    pub value_make_f64: unsafe extern "C" fn(value: f64) -> KValue,
    pub value_make_range: unsafe extern "C" fn(value: KotoRange) -> KValue,
    pub string_make: unsafe extern "C" fn(value: KStringSlice) -> KString,
    pub tuple_make: unsafe extern "C" fn(values: *const KValue, len: usize) -> KTuple,
    pub map_make: unsafe extern "C" fn(entries: *const KotoMapEntry, len: usize) -> KMap,
    pub object_make: unsafe extern "C" fn(
        object_v1: *const KotoPluginObjectV1,
        object_data: KotoObjectDataV1,
    ) -> KObject,
    pub value_clone: unsafe extern "C" fn(value: KValue) -> KValue,
    pub value_free: unsafe extern "C" fn(value: KValue),
    pub value_view_clone: unsafe extern "C" fn(value: KValueView) -> KValue,
    pub value_is_same_instance: unsafe extern "C" fn(a: KValue, b: KValue) -> bool,
    pub value_kind: unsafe extern "C" fn(value: KValue) -> KValueKind,
    pub value_as_bool: unsafe extern "C" fn(value: KValue) -> bool,
    pub value_as_i64: unsafe extern "C" fn(value: KValue) -> i64,
    pub value_as_f64: unsafe extern "C" fn(value: KValue) -> f64,
    pub value_as_range: unsafe extern "C" fn(value: KValue) -> KotoRange,
    pub string_as_slice: unsafe extern "C" fn(string: KString) -> KStringSlice,
    pub tuple_len: unsafe extern "C" fn(tuple: KTuple) -> usize,
    pub tuple_data: unsafe extern "C" fn(tuple: KTuple) -> KValueSlice,
    pub tuple_get: unsafe extern "C" fn(tuple: KTuple, index: usize) -> KValue,
    pub map_len: unsafe extern "C" fn(map: KMap) -> usize,
    pub map_data: unsafe extern "C" fn(map: KMap) -> KMapData,
    pub map_data_get_entry: unsafe extern "C" fn(map: KMapData, index: usize) -> KMapEntryView,
    pub map_key_at: unsafe extern "C" fn(map: KMap, index: usize) -> KValue,
    pub map_value_at: unsafe extern "C" fn(map: KMap, index: usize) -> KValue,
    pub object_v1: unsafe extern "C" fn(object: KObject) -> *const KotoPluginObjectV1,
    pub object_borrow: unsafe extern "C" fn(object: KObject) -> KObjectBorrow,
    pub object_borrow_mut: unsafe extern "C" fn(object: KObject) -> KObjectBorrowMut,
    pub object_borrow_free: unsafe extern "C" fn(borrow: KObjectBorrow),
    pub object_borrow_mut_free: unsafe extern "C" fn(borrow: KObjectBorrowMut),
    pub object_borrow_type_string: unsafe extern "C" fn(borrow: KObjectBorrow) -> KString,
    pub object_borrow_named_value: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        key: KStringSlice,
        out: *mut KValue,
        out_found: *mut bool,
    ) -> KotoStatus,
    pub object_borrow_named_value_assign: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        key: KStringSlice,
        value: KValue,
    ) -> KotoStatus,
    pub object_borrow_iterable_kind: unsafe extern "C" fn(borrow: KObjectBorrow) -> IterableKind,
    pub object_borrow_iterator_next: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        out: *mut KValue,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    pub object_borrow_iterator_next_back: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        out: *mut KValue,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    pub object_borrow_display: unsafe extern "C" fn(borrow: KObjectBorrow) -> KString,
    pub object_borrow_size: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        out: *mut usize,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    pub object_borrow_index:
        unsafe extern "C" fn(borrow: KObjectBorrow, index: KValue, out: *mut KValue) -> KotoStatus,
    pub object_borrow_index_assign:
        unsafe extern "C" fn(borrow: KObjectBorrowMut, index: KValue, value: KValue) -> KotoStatus,
    pub object_borrow_is_callable:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut bool) -> KotoStatus,
    pub object_borrow_call: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        ctx: CallContext,
        out: *mut KValue,
    ) -> KotoStatus,
    pub object_borrow_unary_op:
        unsafe extern "C" fn(borrow: KObjectBorrow, op: UnaryOp, out: *mut KValue) -> KotoStatus,
    pub object_borrow_binary_op: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        op: BinaryOp,
        rhs: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    pub object_borrow_binary_op_assign:
        unsafe extern "C" fn(borrow: KObjectBorrowMut, op: BinaryOp, rhs: KValue) -> KotoStatus,
    pub object_borrow_make_iterator:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut KValue) -> KotoStatus,
    pub object_borrow_serialize:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut KValue) -> KotoStatus,
    pub list_make: unsafe extern "C" fn(values: *const KValue, len: usize) -> KList,
    pub list_len: unsafe extern "C" fn(list: KList) -> usize,
    pub list_data: unsafe extern "C" fn(list: KList) -> KValueSlice,
    pub list_get: unsafe extern "C" fn(list: KList, index: usize) -> KValue,
    pub list_set: unsafe extern "C" fn(list: KList, index: usize, item: KValue) -> KotoStatus,
    pub map_swap_indices: unsafe extern "C" fn(map: KMap, a: usize, b: usize) -> KotoStatus,
    pub map_contains_meta_read: unsafe extern "C" fn(map: KMap, op: ReadOp) -> bool,
    pub map_get_meta_read: unsafe extern "C" fn(map: KMap, op: ReadOp) -> KValue,
    pub map_contains_meta_write: unsafe extern "C" fn(map: KMap, op: WriteOp) -> bool,
    pub map_get_meta_write: unsafe extern "C" fn(map: KMap, op: WriteOp) -> KValue,
    pub vm_call_function: unsafe extern "C" fn(
        function: KValue,
        args: *const KValue,
        arg_count: usize,
        out: *mut KValue,
    ) -> KotoStatus,
    pub vm_call_instance_function: unsafe extern "C" fn(
        instance: KValue,
        function: KValue,
        args: *const KValue,
        arg_count: usize,
        out: *mut KValue,
    ) -> KotoStatus,
    pub vm_run_unary_op:
        unsafe extern "C" fn(op: UnaryOp, value: KValue, out: *mut KValue) -> KotoStatus,
    pub vm_run_binary_op: unsafe extern "C" fn(
        op: BinaryOp,
        lhs: KValue,
        rhs: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    pub vm_run_read_op: unsafe extern "C" fn(
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    pub vm_run_write_op: unsafe extern "C" fn(
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
}
