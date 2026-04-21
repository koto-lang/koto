//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    CallArgs, CallContext, DisplayContext, IsIterable, KCell, KIterator, KIteratorOutput, KList,
    KMap, KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KValue, KotoField, KotoFile,
    KotoFunction, KotoHasher, KotoIterator, KotoObject, KotoRead, KotoSend, KotoSync, KotoVm,
    KotoVmSettings, KotoWrite, MetaKey, MetaMap, MethodContext, RuntimeBackend, ValueKey, ValueMap,
    ValueVec,
    api::{
        BinaryOp, KotoAccess, KotoBackend, KotoCallContext, KotoCollection, KotoCopy, KotoDisplay,
        KotoIdentity, KotoIndexSwap, KotoIteratorBuilder, KotoMap, KotoMapBuilder, KotoMapLookup,
        KotoMapSource, KotoMapSourceMut, KotoMetaMap, KotoMethodContext, KotoNamedAccess,
        KotoNumber, KotoObjectCast, KotoObjectHandle, KotoObjectIterable, KotoObjectOps, KotoRange,
        KotoSequence, KotoSequenceMut, KotoSlice, KotoSliceMut, KotoStaticType, KotoString,
        KotoType, KotoValue, KotoVmTrait, ReadOp, UnaryOp, WriteOp,
    },
    derive::koto_fn,
    make_ptr, make_ptr_mut, runtime_error, unexpected_args, unexpected_args_after_instance,
    unexpected_type,
};
