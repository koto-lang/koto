//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

mod display_context;
mod error;
mod io;
mod types;
mod vm;

pub mod core_lib;
pub mod prelude;
mod send_sync;

pub use crate::{
    display_context::DisplayContext,
    error::{
        Error, ErrorKind, InstructionFrame, Result, unexpected_args,
        unexpected_args_after_instance, unexpected_type,
    },
    io::{
        BufferedFile, KotoFile, KotoRead, KotoWrite, SystemStderr, SystemStdin, SystemStdout,
        UnavailableStderr, UnavailableStdin, UnavailableStdout,
    },
    send_sync::{KotoSend, KotoSync},
    types::{
        ActiveTasks, AsyncKotoVm, BinaryOp, CallContext, FunctionOutput, IsIterable, KFunction,
        KIterator, KIteratorNext, KIteratorOutput, KList, KMap, KNativeFunction, KNativeVmFunction,
        KNumber, KObject, KRange, KString, KTask, KTaskPoll, KTuple, KValue, KotoAccess, KotoCopy,
        KotoField, KotoFunction, KotoFuture, KotoHasher, KotoIterator, KotoObject,
        KotoTaskExecutor, KotoType, KotoVmFunction, LocalTaskExecutor, MetaKey, MetaMap,
        MethodContext, ReadOp, UnaryOp, ValueKey, ValueMap, ValueVec, VmCallContext, WriteOp,
    },
    vm::{CallArgs, KotoVm, KotoVmSettings, ModuleImportedCallback, ReturnOrYield, VmOutput},
};
pub use koto_derive as derive;
pub use koto_memory::{Borrow, BorrowMut, KCell, Ptr, PtrMut, lazy, make_ptr, make_ptr_mut};

#[doc(hidden)]
pub mod __private;
