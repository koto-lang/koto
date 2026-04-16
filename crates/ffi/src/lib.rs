//! Stable ABI definitions for dynamic Koto plugins

#![warn(missing_docs)]

use std::{
    ffi::{c_char, c_void},
    fmt,
    mem::transmute,
};

/// The current ABI major version.
pub const ABI_MAJOR_VERSION: u16 = 1;

/// The current ABI minor version.
pub const ABI_MINOR_VERSION: u16 = 23;

/// An opaque two-word handle to a runtime-owned object value.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KObject {
    /// The data pointer.
    pub data: *mut c_void,
    /// The metadata pointer.
    pub metadata: *mut c_void,
}

/// An opaque two-word handle used for runtime-owned fat-pointer values.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpaqueHandle {
    /// The data pointer.
    pub data: *mut c_void,
    /// The metadata pointer.
    pub metadata: *mut c_void,
}

/// An opaque handle to a runtime-owned list instance.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KList(pub *mut c_void);

/// A typed handle to a runtime-owned map instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KMap {
    /// The shared map data storage.
    pub data: *mut c_void,
    /// The optional shared meta map storage.
    pub meta: *mut c_void,
}

/// The encoded representation used by tuple values in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KTupleKind {
    /// A full tuple using the entire shared backing store.
    Full,
    /// A tuple slice with 16-bit bounds.
    Slice16,
    /// A tuple slice with usize bounds.
    Slice,
}

/// A 16-bit tuple slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KTupleSlice16 {
    /// The shared tuple backing store.
    pub data: *mut c_void,
    /// The inclusive start bound.
    pub start: u16,
    /// The exclusive end bound.
    pub end: u16,
}

/// A usize tuple slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KTupleSlice {
    /// The shared tuple backing store.
    pub data: *mut c_void,
    /// The inclusive start bound.
    pub start: usize,
    /// The exclusive end bound.
    pub end: usize,
}

/// The tagged payload for a tuple value.
#[repr(C)]
#[derive(Clone, Copy)]
pub union KTupleData {
    /// The full tuple payload.
    pub full: *mut c_void,
    /// The 16-bit slice payload.
    pub slice16: KTupleSlice16,
    /// The usize slice payload.
    pub slice: KTupleSlice,
}

/// A tuple value passed across the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KTuple {
    /// The tuple representation kind.
    pub kind: KTupleKind,
    /// The tagged tuple payload.
    pub data: KTupleData,
}

impl fmt::Debug for KTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KotoTuple")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Default for KTuple {
    fn default() -> Self {
        Self {
            kind: KTupleKind::Full,
            data: KTupleData {
                full: std::ptr::null_mut(),
            },
        }
    }
}

/// The number of machine words reserved in [`KObjectBorrow`] for runtime-private token data.
pub const KOBJECT_BORROW_WORDS: usize = 4;

/// An opaque token representing an active borrow of plugin object data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KObjectBorrow {
    /// The borrowed plugin object data pointer.
    pub data: *mut c_void,
    /// Opaque runtime-private borrow token storage.
    pub storage: [usize; KOBJECT_BORROW_WORDS],
}

impl KObjectBorrow {
    /// Returns `true` when this token contains an active borrow.
    pub const fn is_valid(self) -> bool {
        !self.data.is_null()
    }
}

impl Default for KObjectBorrow {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            storage: [0; KOBJECT_BORROW_WORDS],
        }
    }
}

/// An opaque token representing an active mutable borrow of plugin object data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KObjectBorrowMut {
    /// The borrowed plugin object data pointer.
    pub data: *mut c_void,
    /// Opaque runtime-private borrow token storage.
    pub storage: [usize; KOBJECT_BORROW_WORDS],
}

impl KObjectBorrowMut {
    /// Returns `true` when this token contains an active mutable borrow.
    pub const fn is_valid(self) -> bool {
        !self.data.is_null()
    }

    /// Returns this mutable borrow token as a shared borrow token.
    pub const fn as_shared(self) -> KObjectBorrow {
        KObjectBorrow {
            data: self.data,
            storage: self.storage,
        }
    }
}

impl Default for KObjectBorrowMut {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            storage: [0; KOBJECT_BORROW_WORDS],
        }
    }
}

/// A borrowed UTF-8 string slice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KStringSlice {
    /// The string data.
    pub ptr: *const u8,
    /// The string length in bytes.
    pub len: usize,
}

/// The encoded representation used by string values in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KStringKind {
    /// A full string using the entire shared backing store.
    Full,
    /// A string slice with 16-bit bounds.
    Slice16,
    /// A string slice with usize bounds.
    Slice,
}

/// A 16-bit string slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KStringBounds16 {
    /// The shared string backing store.
    pub data: *mut c_void,
    /// The inclusive start bound in bytes.
    pub start: u16,
    /// The exclusive end bound in bytes.
    pub end: u16,
}

/// A usize string slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KStringBounds {
    /// The shared string backing store.
    pub data: *mut c_void,
    /// The inclusive start bound in bytes.
    pub start: usize,
    /// The exclusive end bound in bytes.
    pub end: usize,
}

/// The tagged payload for a string value.
#[repr(C)]
#[derive(Clone, Copy)]
pub union KStringData {
    /// The full string payload.
    pub full: *mut c_void,
    /// The 16-bit slice payload.
    pub slice16: KStringBounds16,
    /// The usize slice payload.
    pub slice: KStringBounds,
}

/// A string value passed across the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KString {
    /// The string representation kind.
    pub kind: KStringKind,
    /// The tagged string payload.
    pub data: KStringData,
}

impl Default for KString {
    fn default() -> Self {
        Self {
            kind: KStringKind::Full,
            data: KStringData {
                full: std::ptr::null_mut(),
            },
        }
    }
}

/// The supported value kinds in the v1 plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KValueKind {
    /// The null value.
    Null,
    /// A boolean value.
    Bool,
    /// A signed 64-bit integer.
    I64,
    /// A 64-bit floating point number.
    F64,
    /// A range value.
    Range,
    /// A UTF-8 string.
    String,
    /// A list value.
    List,
    /// A tuple value.
    Tuple,
    /// A map value.
    Map,
    /// A Koto function.
    Function,
    /// A native function.
    NativeFunction,
    /// An iterator value.
    Iterator,
    /// A plugin-owned object value.
    Object,
    /// A runtime type that the current ABI doesn't expose.
    Unsupported,
}

/// A status code returned by ABI calls.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KotoStatusCode {
    /// The operation succeeded.
    Ok = 0,
    /// The operation failed.
    Error = 1,
}

/// Clones a runtime-owned error handle stored in [`KotoStatus`].
pub type StatusFnCloneError = unsafe extern "C" fn(error: *mut c_void) -> *mut c_void;

/// Frees a runtime-owned error handle stored in [`KotoStatus`].
pub type StatusFnFreeError = unsafe extern "C" fn(error: *mut c_void);

/// The unary operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryOp {
    /// `@debug`
    Debug,
    /// `@display`
    Display,
    /// `@negate`
    Negate,
    /// `@iterator`
    Iterator,
    /// `@next`
    Next,
    /// `@next_back`
    NextBack,
    /// `@size`
    Size,
}

/// The binary operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    /// `@+`
    Add,
    /// `@-`
    Subtract,
    /// `@*`
    Multiply,
    /// `@/`
    Divide,
    /// `@%`
    Remainder,
    /// `@^`
    Power,
    /// `@r+`
    AddRhs,
    /// `@r-`
    SubtractRhs,
    /// `@r*`
    MultiplyRhs,
    /// `@r/`
    DivideRhs,
    /// `@r%`
    RemainderRhs,
    /// `@r^`
    PowerRhs,
    /// `@+=`
    AddAssign,
    /// `@-=`
    SubtractAssign,
    /// `@*=`
    MultiplyAssign,
    /// `@/=`
    DivideAssign,
    /// `@%=`
    RemainderAssign,
    /// `@^=`
    PowerAssign,
    /// `@<`
    Less,
    /// `@<=`
    LessOrEqual,
    /// `@>`
    Greater,
    /// `@>=`
    GreaterOrEqual,
    /// `@==`
    Equal,
    /// `@!=`
    NotEqual,
}

/// The read operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadOp {
    /// `@index`
    Index,
    /// `@access`
    Access,
}

/// The write operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WriteOp {
    /// `@index_assign`
    IndexAssign,
    /// `@access_assign`
    AccessAssign,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;

        write!(
            f,
            "{}",
            match self {
                Add | AddRhs => "+",
                Subtract | SubtractRhs => "-",
                Multiply | MultiplyRhs => "*",
                Divide | DivideRhs => "/",
                Remainder | RemainderRhs => "%",
                Power | PowerRhs => "^",
                AddAssign => "+=",
                SubtractAssign => "-=",
                MultiplyAssign => "*=",
                DivideAssign => "/=",
                RemainderAssign => "%=",
                PowerAssign => "^=",
                Less => "<",
                LessOrEqual => "<=",
                Greater => ">",
                GreaterOrEqual => ">=",
                Equal => "==",
                NotEqual => "!=",
            }
        )
    }
}

impl fmt::Display for ReadOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ReadOp::Index => "[]",
                ReadOp::Access => ".",
            }
        )
    }
}

impl fmt::Display for WriteOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WriteOp::IndexAssign => "[]",
                WriteOp::AccessAssign => ".",
            }
        )
    }
}

/// The kinds of meta keys supported by the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetaKeyKind {
    /// A unary operation.
    UnaryOp,
    /// A binary operation.
    BinaryOp,
    /// A read operation.
    ReadOp,
    /// A write operation.
    WriteOp,
    /// `@call`
    Call,
    /// `@meta name`
    Named,
    /// `@test name`
    Test,
    /// `@pre_test`
    PreTest,
    /// `@post_test`
    PostTest,
    /// `@main`
    Main,
    /// `@type`
    Type,
    /// `@base`
    Base,
}

/// A meta key used by map-building callbacks in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub union MetaKeyData {
    /// The unary operation payload.
    pub unary_op: UnaryOp,
    /// The binary operation payload.
    pub binary_op: BinaryOp,
    /// The read operation payload.
    pub read_op: ReadOp,
    /// The write operation payload.
    pub write_op: WriteOp,
    /// The string payload for named keys.
    pub string: KStringSlice,
}

/// A meta key used by map-building callbacks in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MetaKey {
    /// The key kind.
    pub kind: MetaKeyKind,
    /// The tagged payload for the key kind.
    pub data: MetaKeyData,
}

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
            // Safety: `clone_error` is only populated from a `StatusFnCloneError` cast to
            // `*const c_void` when the status is constructed. A null pointer is handled above,
            // so this is a round-trip back to the original function-pointer type.
            Some(unsafe { transmute::<*const c_void, StatusFnCloneError>(self.clone_error) })
        }
    }

    /// Returns [`Self::free_error`] as a function pointer, when present.
    pub fn free_error_fn(self) -> Option<StatusFnFreeError> {
        if self.free_error.is_null() {
            None
        } else {
            // Safety: `free_error` is only populated from a `StatusFnFreeError` cast to
            // `*const c_void` when the status is constructed. A null pointer is handled above,
            // so this is a round-trip back to the original function-pointer type.
            Some(unsafe { transmute::<*const c_void, StatusFnFreeError>(self.free_error) })
        }
    }
}

/// A key/value entry used when constructing maps.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KotoMapEntry {
    /// The key value.
    pub key: KValue,
    /// The value.
    pub value: KValue,
}

/// A range value used in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KotoRange {
    /// The range start, if present.
    pub start: i64,
    /// True when the range has a start bound.
    pub has_start: bool,
    /// The range end, if present.
    pub end: i64,
    /// True when the range has an end bound.
    pub has_end: bool,
    /// True when the end bound is inclusive.
    pub end_inclusive: bool,
}

/// The encoded representation of a runtime Koto function.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KFunction {
    /// The shared function chunk storage.
    pub chunk: *mut c_void,
    /// The function's start instruction pointer.
    pub ip: u32,
    /// The function's argument count.
    pub arg_count: u8,
    /// The number of optional arguments.
    pub optional_arg_count: u8,
    /// The encoded function flags.
    pub flags: u8,
    /// Padding reserved for future ABI expansion.
    pub _reserved: u8,
    /// The optional shared function-context storage.
    pub context: *mut c_void,
}

/// The tagged payload for a Koto value.
#[repr(C)]
#[derive(Clone, Copy)]
pub union KValueData {
    /// The bool payload.
    pub bool_value: bool,
    /// The i64 payload.
    pub i64_value: i64,
    /// The f64 payload.
    pub f64_value: f64,
    /// The range payload.
    pub range_value: KotoRange,
    /// The string payload.
    pub string_value: KString,
    /// The tuple payload.
    pub tuple_value: KTuple,
    /// The map payload.
    pub map_value: KMap,
    /// The function payload.
    pub function_value: KFunction,
    /// The native-function payload.
    pub native_function_value: OpaqueHandle,
    /// The iterator payload.
    pub iterator_value: OpaqueHandle,
    /// The object payload.
    pub object_value: KObject,
    /// The handle payload for runtime-owned values.
    pub handle: *mut c_void,
}

/// A Koto value passed across the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KValue {
    /// The value kind.
    pub kind: KValueKind,
    /// The tagged payload.
    pub data: KValueData,
}

impl fmt::Debug for KValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KotoValue")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl KValue {
    /// Returns the null value.
    pub const fn null() -> Self {
        Self {
            kind: KValueKind::Null,
            data: KValueData { i64_value: 0 },
        }
    }

    /// Returns true if this value uses a runtime-owned handle.
    pub const fn is_handle(self) -> bool {
        matches!(
            self.kind,
            KValueKind::List | KValueKind::NativeFunction | KValueKind::Iterator
        )
    }
}

impl Default for KValue {
    fn default() -> Self {
        Self::null()
    }
}

/// A borrowed view of a runtime-owned value.
///
/// The pointed-to value remains owned by the runtime. Plugins must not attempt to interpret the
/// pointer directly; it is only valid to pass the view back through host API callbacks.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KValueView(pub *const c_void);

/// A borrowed view of a contiguous value slice.
///
/// The slice data remains owned by the runtime and is valid only while the originating container
/// remains alive. Plugins must not inspect the pointed-to bytes directly; they may only use the
/// slice metadata to construct [`KValueView`]s that are passed back to the host API.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KValueSlice {
    /// The pointer to the first runtime-owned value in the slice.
    pub data: *const c_void,
    /// The number of values in the slice.
    pub len: usize,
    /// The byte stride between adjacent values.
    pub stride: usize,
}

impl KValueSlice {
    /// Returns `true` if the slice is empty.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// A borrowed view of map data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KMapData {
    /// The opaque runtime-owned map data pointer.
    pub data: *const c_void,
    /// The number of entries in the map.
    pub len: usize,
}

impl KMapData {
    /// Returns `true` if the map data is empty.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// A borrowed view of a map entry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KMapEntryView {
    /// A borrowed view of the entry key.
    pub key: KValueView,
    /// A borrowed view of the entry value.
    pub value: KValueView,
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

/// Indicates whether a plugin-owned object is iterable.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IterableKind {
    /// The object isn't iterable.
    NotIterable,
    /// The object is iterable and can produce an iterator.
    Iterable,
    /// The object is a forward iterator.
    ForwardIterator,
    /// The object is a bidirectional iterator.
    BidirectionalIterator,
}

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
///
/// The plugin ABI uses a versioned function table instead of a flat exported symbol set so the
/// runtime can pass one capability object into the plugin entrypoint and extend it compatibly over
/// time.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KotoHostApiV1 {
    /// The ABI major version.
    pub abi_major: u16,
    /// The ABI minor version.
    pub abi_minor: u16,
    /// The size of this struct.
    pub struct_size: usize,
    /// Creates a map with the given `@type`.
    pub map_new_with_type: unsafe extern "C" fn(type_name: KStringSlice) -> KMap,
    /// Inserts a value into a map and consumes the value handle.
    pub map_insert_value: unsafe extern "C" fn(map: KMap, key: KStringSlice, value: KValue),
    /// Inserts a meta value into a map and consumes the value handle.
    pub map_insert_meta_value: unsafe extern "C" fn(map: KMap, key: MetaKey, value: KValue),
    /// Creates a native function value backed by a plugin callback.
    pub native_function_make: unsafe extern "C" fn(
        function: KotoPluginFunction,
        user_data: *mut c_void,
        drop_user_data: KotoPluginDrop,
    ) -> OpaqueHandle,
    /// Creates a null value.
    pub value_make_null: unsafe extern "C" fn() -> KValue,
    /// Creates a boolean value.
    pub value_make_bool: unsafe extern "C" fn(value: bool) -> KValue,
    /// Creates an integer value.
    pub value_make_i64: unsafe extern "C" fn(value: i64) -> KValue,
    /// Creates a floating point value.
    pub value_make_f64: unsafe extern "C" fn(value: f64) -> KValue,
    /// Creates a range value.
    pub value_make_range: unsafe extern "C" fn(value: KotoRange) -> KValue,
    /// Creates a string handle.
    pub string_make: unsafe extern "C" fn(value: KStringSlice) -> KString,
    /// Creates a tuple handle and consumes the provided value handles.
    pub tuple_make: unsafe extern "C" fn(values: *const KValue, len: usize) -> KTuple,
    /// Creates a map handle and consumes the provided entry handles.
    pub map_make: unsafe extern "C" fn(entries: *const KotoMapEntry, len: usize) -> KMap,
    /// Creates and initializes a runtime-owned plugin object value.
    pub object_make: unsafe extern "C" fn(
        object_v1: *const KotoPluginObjectV1,
        object_data: KotoObjectDataV1,
    ) -> KObject,
    /// Clones a value handle.
    pub value_clone: unsafe extern "C" fn(value: KValue) -> KValue,
    /// Frees a value handle that won't be consumed elsewhere.
    pub value_free: unsafe extern "C" fn(value: KValue),
    /// Clones a borrowed value view into an owned value handle.
    pub value_view_clone: unsafe extern "C" fn(value: KValueView) -> KValue,
    /// Returns `true` if both values refer to the same underlying runtime instance.
    pub value_is_same_instance: unsafe extern "C" fn(a: KValue, b: KValue) -> bool,
    /// Returns the value kind.
    pub value_kind: unsafe extern "C" fn(value: KValue) -> KValueKind,
    /// Returns the boolean contents of a bool value.
    pub value_as_bool: unsafe extern "C" fn(value: KValue) -> bool,
    /// Returns the integer contents of an i64 value.
    pub value_as_i64: unsafe extern "C" fn(value: KValue) -> i64,
    /// Returns the float contents of an f64 value.
    pub value_as_f64: unsafe extern "C" fn(value: KValue) -> f64,
    /// Returns a range value.
    pub value_as_range: unsafe extern "C" fn(value: KValue) -> KotoRange,
    /// Returns a borrowed view of a string handle.
    pub string_as_slice: unsafe extern "C" fn(string: KString) -> KStringSlice,
    /// Returns the length of a tuple.
    pub tuple_len: unsafe extern "C" fn(tuple: KTuple) -> usize,
    /// Returns a borrowed view of a tuple's data.
    pub tuple_data: unsafe extern "C" fn(tuple: KTuple) -> KValueSlice,
    /// Returns a cloned tuple item at the given index.
    pub tuple_get: unsafe extern "C" fn(tuple: KTuple, index: usize) -> KValue,
    /// Returns the number of entries in a map.
    pub map_len: unsafe extern "C" fn(map: KMap) -> usize,
    /// Returns a borrowed view of a map's data.
    pub map_data: unsafe extern "C" fn(map: KMap) -> KMapData,
    /// Returns a borrowed view of the entry at the given insertion index.
    pub map_data_get_entry: unsafe extern "C" fn(map: KMapData, index: usize) -> KMapEntryView,
    /// Returns a cloned map key at the given index.
    pub map_key_at: unsafe extern "C" fn(map: KMap, index: usize) -> KValue,
    /// Returns a cloned map value at the given index.
    pub map_value_at: unsafe extern "C" fn(map: KMap, index: usize) -> KValue,
    /// Returns the plugin object descriptor for an object value, or null if unavailable.
    pub object_v1: unsafe extern "C" fn(object: KObject) -> *const KotoPluginObjectV1,
    /// Attempts to immutably borrow the storage owned by a plugin object value.
    pub object_borrow: unsafe extern "C" fn(object: KObject) -> KObjectBorrow,
    /// Attempts to mutably borrow the storage owned by a plugin object value.
    pub object_borrow_mut: unsafe extern "C" fn(object: KObject) -> KObjectBorrowMut,
    /// Releases an active plugin object borrow.
    pub object_borrow_free: unsafe extern "C" fn(borrow: KObjectBorrow),
    /// Releases an active plugin object mutable borrow.
    pub object_borrow_mut_free: unsafe extern "C" fn(borrow: KObjectBorrowMut),
    /// Returns a borrowed object's type name.
    pub object_borrow_type_string: unsafe extern "C" fn(borrow: KObjectBorrow) -> KString,
    /// Looks up a named value on a borrowed object.
    pub object_borrow_named_value: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        key: KStringSlice,
        out: *mut KValue,
        out_found: *mut bool,
    ) -> KotoStatus,
    /// Assigns a named value on a mutably borrowed object.
    pub object_borrow_named_value_assign: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        key: KStringSlice,
        value: KValue,
    ) -> KotoStatus,
    /// Returns whether a borrowed object is iterable.
    pub object_borrow_iterable_kind: unsafe extern "C" fn(borrow: KObjectBorrow) -> IterableKind,
    /// Returns the next value from a borrowed iterator object.
    pub object_borrow_iterator_next: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        out: *mut KValue,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    /// Returns the next value from the end of a borrowed bidirectional iterator object.
    pub object_borrow_iterator_next_back: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        out: *mut KValue,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    /// Returns a borrowed object's display string.
    pub object_borrow_display: unsafe extern "C" fn(borrow: KObjectBorrow) -> KString,
    /// Returns a borrowed object's size, if any.
    pub object_borrow_size: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        out: *mut usize,
        out_has_value: *mut bool,
    ) -> KotoStatus,
    /// Indexes a borrowed object.
    pub object_borrow_index:
        unsafe extern "C" fn(borrow: KObjectBorrow, index: KValue, out: *mut KValue) -> KotoStatus,
    /// Assigns via indexing on a mutably borrowed object.
    pub object_borrow_index_assign:
        unsafe extern "C" fn(borrow: KObjectBorrowMut, index: KValue, value: KValue) -> KotoStatus,
    /// Returns whether a borrowed object is callable.
    pub object_borrow_is_callable:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut bool) -> KotoStatus,
    /// Calls a mutably borrowed object.
    pub object_borrow_call: unsafe extern "C" fn(
        borrow: KObjectBorrowMut,
        ctx: CallContext,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Applies a unary operation to a borrowed object.
    pub object_borrow_unary_op:
        unsafe extern "C" fn(borrow: KObjectBorrow, op: UnaryOp, out: *mut KValue) -> KotoStatus,
    /// Applies a binary operation to a borrowed object.
    pub object_borrow_binary_op: unsafe extern "C" fn(
        borrow: KObjectBorrow,
        op: BinaryOp,
        rhs: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Applies an in-place binary operation to a mutably borrowed object.
    pub object_borrow_binary_op_assign:
        unsafe extern "C" fn(borrow: KObjectBorrowMut, op: BinaryOp, rhs: KValue) -> KotoStatus,
    /// Produces an iterator value for a borrowed iterable object.
    pub object_borrow_make_iterator:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut KValue) -> KotoStatus,
    /// Serializes a borrowed object into a value.
    pub object_borrow_serialize:
        unsafe extern "C" fn(borrow: KObjectBorrow, out: *mut KValue) -> KotoStatus,
    /// Creates a list handle and consumes the provided value handles.
    pub list_make: unsafe extern "C" fn(values: *const KValue, len: usize) -> KList,
    /// Returns the length of a list.
    pub list_len: unsafe extern "C" fn(list: KList) -> usize,
    /// Returns a borrowed view of a list's data.
    pub list_data: unsafe extern "C" fn(list: KList) -> KValueSlice,
    /// Returns a cloned list item at the given index.
    pub list_get: unsafe extern "C" fn(list: KList, index: usize) -> KValue,
    /// Replaces a list item at the given index.
    pub list_set: unsafe extern "C" fn(list: KList, index: usize, item: KValue) -> KotoStatus,
    /// Swaps two map entries by index.
    pub map_swap_indices: unsafe extern "C" fn(map: KMap, a: usize, b: usize) -> KotoStatus,
    /// Returns true if the map contains a read-op meta entry.
    pub map_contains_meta_read: unsafe extern "C" fn(map: KMap, op: ReadOp) -> bool,
    /// Returns the map's read-op meta value, or null if absent.
    pub map_get_meta_read: unsafe extern "C" fn(map: KMap, op: ReadOp) -> KValue,
    /// Returns true if the map contains a write-op meta entry.
    pub map_contains_meta_write: unsafe extern "C" fn(map: KMap, op: WriteOp) -> bool,
    /// Returns the map's write-op meta value, or null if absent.
    pub map_get_meta_write: unsafe extern "C" fn(map: KMap, op: WriteOp) -> KValue,
    /// Calls a function using the active VM.
    pub vm_call_function: unsafe extern "C" fn(
        function: KValue,
        args: *const KValue,
        arg_count: usize,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Calls an instance function using the active VM.
    pub vm_call_instance_function: unsafe extern "C" fn(
        instance: KValue,
        function: KValue,
        args: *const KValue,
        arg_count: usize,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Runs a unary operation using the active VM.
    pub vm_run_unary_op:
        unsafe extern "C" fn(op: UnaryOp, value: KValue, out: *mut KValue) -> KotoStatus,
    /// Runs a binary operation using the active VM.
    pub vm_run_binary_op: unsafe extern "C" fn(
        op: BinaryOp,
        lhs: KValue,
        rhs: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Runs a read operation using the active VM.
    pub vm_run_read_op: unsafe extern "C" fn(
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
    /// Runs a write operation using the active VM.
    pub vm_run_write_op: unsafe extern "C" fn(
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
        out: *mut KValue,
    ) -> KotoStatus,
}
