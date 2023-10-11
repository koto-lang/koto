//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_runtime_error, runtime_error, type_error, type_error_with_slice, BinaryOp, Borrow,
    BorrowMut, CallArgs, CallContext, DisplayContext, ExternalFunction, IsIterable, KIterator,
    KIteratorOutput, KList, KMap, KNumber, KRange, KString, KTuple, KotoFile, KotoHasher,
    KotoIterator, KotoObject, KotoRead, KotoType, KotoWrite, MetaKey, MetaMap, Object,
    ObjectEntryBuilder, Ptr, PtrMut, RuntimeError, UnaryOp, Value, ValueKey, ValueMap, ValueVec,
    Vm, VmSettings,
};
