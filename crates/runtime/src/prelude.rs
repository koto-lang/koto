//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_ptr, make_ptr_mut, runtime_error, unexpected_args, unexpected_args_after_instance,
    unexpected_type, BinaryOp, CallArgs, CallContext, DisplayContext, IsIterable, KCell, KIterator,
    KIteratorOutput, KList, KMap, KNativeFunction, KNumber, KObject, KRange, KString, KTuple,
    KValue, KotoCopy, KotoEntries, KotoFile, KotoFunction, KotoHasher, KotoIterator, KotoObject,
    KotoRead, KotoSend, KotoSync, KotoType, KotoVm, KotoVmSettings, KotoWrite, MetaKey, MetaMap,
    MethodContext, UnaryOp, ValueKey, ValueMap, ValueVec,
};
