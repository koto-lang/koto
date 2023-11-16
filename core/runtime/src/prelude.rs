//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    runtime_error, type_error, type_error_with_slice, BinaryOp, Borrow, BorrowMut, CallArgs,
    CallContext, DisplayContext, IsIterable, KIterator, KIteratorOutput, KList, KMap,
    KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KotoFile, KotoHasher, KotoIterator,
    KotoObject, KotoRead, KotoType, KotoWrite, MetaKey, MetaMap, ObjectEntryBuilder, Ptr, PtrMut,
    UnaryOp, Value, ValueKey, ValueMap, ValueVec, Vm, VmSettings,
};
