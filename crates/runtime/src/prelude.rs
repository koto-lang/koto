//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_ptr, make_ptr_mut, runtime_error, type_error, type_error_with_slice, BinaryOp, Borrow,
    BorrowMut, CallArgs, CallContext, DisplayContext, IsIterable, KCell, KIterator,
    KIteratorOutput, KList, KMap, KNativeFunction, KNumber, KObject, KRange, KString, KTuple,
    KValue, KotoCopy, KotoFile, KotoFunction, KotoHasher, KotoIterator, KotoLookup, KotoObject,
    KotoRead, KotoSend, KotoSync, KotoType, KotoWrite, MetaKey, MetaMap, MethodContext, Ptr,
    PtrMut, UnaryOp, ValueKey, ValueMap, ValueVec, Vm, VmSettings,
};
