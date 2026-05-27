//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    ActiveTasks, AsyncKotoVm, BinaryOp, CallArgs, CallContext, DisplayContext, FunctionOutput,
    IsIterable, KCell, KIterator, KIteratorNext, KIteratorOutput, KList, KMap, KNativeFunction,
    KNativeVmFunction, KNumber, KObject, KRange, KString, KTask, KTaskPoll, KTuple, KValue,
    KotoAccess, KotoCopy, KotoField, KotoFile, KotoFunction, KotoFuture, KotoHasher, KotoIterator,
    KotoObject, KotoRead, KotoSend, KotoSync, KotoTaskExecutor, KotoType, KotoVm, KotoVmFunction,
    KotoVmSettings, KotoWrite, LocalTaskExecutor, MetaKey, MetaMap, MethodContext, ReadOp, UnaryOp,
    ValueKey, ValueMap, ValueVec, VmCallContext, VmOutput, WriteOp, derive::koto_fn, make_ptr,
    make_ptr_mut, runtime_error, unexpected_args, unexpected_args_after_instance, unexpected_type,
};
