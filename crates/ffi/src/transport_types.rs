//! Shared transport layout instantiated by both the native and wasm ABI modules.
//!
//! This file expects the parent module to define the pointer/size aliases it imports below,
//! which lets `native` and `wasm` share one set of transport type declarations without macros
//! or duplicated struct definitions.

use super::{BytePtr, ConstPtr, MutPtr, Size, Word, is_null_mut_ptr};
use crate::shared::{BinaryOp, KValueKind, MetaKeyKind, ReadOp, UnaryOp, WriteOp};

/// An opaque two-word handle to a runtime-owned object value.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KObject {
    /// The data pointer.
    pub data: MutPtr,
    /// The metadata pointer.
    pub metadata: MutPtr,
}

/// An opaque two-word handle used for runtime-owned fat-pointer values.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpaqueHandle {
    /// The data pointer.
    pub data: MutPtr,
    /// The metadata pointer.
    pub metadata: MutPtr,
}

/// An opaque handle to a runtime-owned list instance.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KList(pub MutPtr);

/// A typed handle to a runtime-owned map instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KMap {
    /// The shared map data storage.
    pub data: MutPtr,
    /// The optional shared meta map storage.
    pub meta: MutPtr,
}

/// The encoded representation used by tuple values in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KTupleKind {
    /// A full tuple using the entire shared backing store.
    Full,
    /// A tuple slice with 16-bit bounds.
    Slice16,
    /// A tuple slice with size bounds.
    Slice,
}

/// A 16-bit tuple slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KTupleSlice16 {
    /// The shared tuple backing store.
    pub data: MutPtr,
    /// The inclusive start bound.
    pub start: u16,
    /// The exclusive end bound.
    pub end: u16,
}

/// A size-based tuple slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KTupleSlice {
    /// The shared tuple backing store.
    pub data: MutPtr,
    /// The inclusive start bound.
    pub start: Size,
    /// The exclusive end bound.
    pub end: Size,
}

/// The tagged payload for a tuple value.
#[repr(C)]
#[derive(Clone, Copy)]
pub union KTupleData {
    /// The full tuple payload.
    pub full: MutPtr,
    /// The 16-bit slice payload.
    pub slice16: KTupleSlice16,
    /// The size-based slice payload.
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

impl ::std::fmt::Debug for KTuple {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
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
                full: Default::default(),
            },
        }
    }
}

/// An opaque token representing an active borrow of plugin object data.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub struct KObjectBorrow {
    /// The borrowed plugin object data pointer.
    pub data: MutPtr,
    /// Opaque runtime-private borrow token storage.
    pub storage: [Word; 4],
}

impl KObjectBorrow {
    /// Returns `true` when this token contains an active borrow.
    pub const fn is_valid(self) -> bool {
        !is_null_mut_ptr(self.data)
    }
}

/// An opaque token representing an active mutable borrow of plugin object data.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub struct KObjectBorrowMut {
    /// The borrowed plugin object data pointer.
    pub data: MutPtr,
    /// Opaque runtime-private borrow token storage.
    pub storage: [Word; 4],
}

impl KObjectBorrowMut {
    /// Returns `true` when this token contains an active mutable borrow.
    pub const fn is_valid(self) -> bool {
        !is_null_mut_ptr(self.data)
    }

    /// Returns this mutable borrow token as a shared borrow token.
    pub const fn as_shared(self) -> KObjectBorrow {
        KObjectBorrow {
            data: self.data,
            storage: self.storage,
        }
    }
}

/// A borrowed UTF-8 string slice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KStringSlice {
    /// The string data.
    pub ptr: BytePtr,
    /// The string length in bytes.
    pub len: Size,
}

/// The encoded representation used by string values in the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KStringKind {
    /// A full string using the entire shared backing store.
    Full,
    /// A string slice with 16-bit bounds.
    Slice16,
    /// A string slice with size bounds.
    Slice,
}

/// A 16-bit string slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KStringBounds16 {
    /// The shared string backing store.
    pub data: MutPtr,
    /// The inclusive start bound in bytes.
    pub start: u16,
    /// The exclusive end bound in bytes.
    pub end: u16,
}

/// A size-based string slice payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KStringBounds {
    /// The shared string backing store.
    pub data: MutPtr,
    /// The inclusive start bound in bytes.
    pub start: Size,
    /// The exclusive end bound in bytes.
    pub end: Size,
}

/// The tagged payload for a string value.
#[repr(C)]
#[derive(Clone, Copy)]
pub union KStringData {
    /// The full string payload.
    pub full: MutPtr,
    /// The 16-bit slice payload.
    pub slice16: KStringBounds16,
    /// The size-based slice payload.
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
                full: Default::default(),
            },
        }
    }
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
    pub chunk: MutPtr,
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
    pub context: MutPtr,
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
    pub handle: MutPtr,
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

impl ::std::fmt::Debug for KValue {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
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
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KValueView(pub ConstPtr);

/// A borrowed view of a contiguous value slice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KValueSlice {
    /// The pointer to the first runtime-owned value in the slice.
    pub data: ConstPtr,
    /// The number of values in the slice.
    pub len: Size,
    /// The byte stride between adjacent values.
    pub stride: Size,
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
    pub data: ConstPtr,
    /// The number of entries in the map.
    pub len: Size,
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

/// A key/value entry used when constructing maps.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KotoMapEntry {
    /// The key value.
    pub key: KValue,
    /// The value.
    pub value: KValue,
}
