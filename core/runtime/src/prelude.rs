//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_runtime_error, runtime_error, type_error, type_error_with_slice, ArgRegisters, BinaryOp,
    Borrow, BorrowMut, CallArgs, DataMap, DisplayContext, IntRange, IsIterable, KotoFile,
    KotoHasher, KotoIterator, KotoObject, KotoRead, KotoType, KotoWrite, MetaKey, MetaMap, Object,
    ObjectEntryBuilder, Ptr, PtrMut, RuntimeError, RuntimeResult, UnaryOp, Value, ValueIterator,
    ValueIteratorOutput, ValueKey, ValueList, ValueMap, ValueNumber, ValueString, ValueTuple,
    ValueVec, Vm, VmSettings,
};
