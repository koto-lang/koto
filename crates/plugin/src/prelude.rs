//! A collection of useful items to make it easier to work with `koto_plugin`

#[doc(inline)]
pub use crate::{
    Borrow, BorrowMut, CallContext, DisplayContext, IsIterable, KFunction, KIterator,
    KIteratorOutput, KList, KMap, KNativeFunction, KNumber, KObject, KRange, KString, KTuple,
    KValue, KotoField, KotoSend, KotoSync, KotoVm, MetaKey, MethodContext, ObjectBorrow,
    ObjectBorrowMut, PluginBackend,
    api::{
        BinaryOp, KotoAccess, KotoBackend, KotoCallContext, KotoCollection, KotoCopy, KotoDisplay,
        KotoIdentity, KotoIndexSwap, KotoIteratorBuilder, KotoMap, KotoMapBuilder, KotoMapLookup,
        KotoMapSource, KotoMapSourceMut, KotoMetaMap, KotoMethodContext, KotoNamedAccess,
        KotoNumber, KotoObjectCast, KotoObjectHandle, KotoObjectIterable, KotoObjectOps, KotoRange,
        KotoSequence, KotoSequenceMut, KotoSlice, KotoSliceMut, KotoStaticType, KotoString,
        KotoType, KotoValue, KotoVmTrait, ReadOp, UnaryOp, WriteOp,
    },
    derive::koto_fn,
    runtime_error, unexpected_args, unexpected_args_after_instance, unexpected_type,
};
