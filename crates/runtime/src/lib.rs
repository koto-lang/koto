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
    io::{BufferedFile, DefaultStderr, DefaultStdin, DefaultStdout, KotoFile, KotoRead, KotoWrite},
    send_sync::{KotoSend, KotoSync},
    types::{
        BinaryOp, CallContext, IsIterable, KFunction, KIterator, KIteratorOutput, KList, KMap,
        KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KValue, KotoAccess, KotoCopy,
        KotoField, KotoFunction, KotoHasher, KotoIterator, KotoObject, KotoType, MetaKey, MetaMap,
        MethodContext, ReadOp, UnaryOp, ValueKey, ValueMap, ValueVec, WriteOp,
    },
    vm::{CallArgs, KotoVm, KotoVmSettings, ModuleImportedCallback, ReturnOrYield},
};
pub use koto_derive as derive;
pub use koto_memory::{Borrow, BorrowMut, KCell, Ptr, PtrMut, lazy, make_ptr, make_ptr_mut};

#[doc(hidden)]
pub mod __private;
