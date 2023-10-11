//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_runtime_error, runtime_error, type_error, type_error_with_slice, BinaryOp, Borrow,
    BorrowMut, CallArgs, CallContext, DisplayContext, ExternalFunction, IntRange, IsIterable, KMap,
    KotoFile, KotoHasher, KotoIterator, KotoObject, KotoRead, KotoType, KotoWrite, MetaKey,
    MetaMap, Object, ObjectEntryBuilder, Ptr, PtrMut, RuntimeError, UnaryOp, Value, ValueIterator,
    ValueIteratorOutput, ValueKey, ValueList, ValueMap, ValueNumber, ValueString, ValueTuple,
    ValueVec, Vm, VmSettings,
};
