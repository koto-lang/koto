//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

#[cfg(all(feature = "plugin", feature = "rc"))]
compile_error!("Dynamic plugin hosting requires the `arc` memory management feature");

mod display_context;
mod error;
mod io;
#[cfg(feature = "plugin")]
mod plugin_host;
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
        CallContext, IsIterable, KFunction, KIterator, KIteratorOutput, KList, KMap,
        KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KValue, KotoField,
        KotoFunction, KotoHasher, KotoIterator, KotoObject, MetaKey, MetaMap, MethodContext,
        RuntimeBackend, ValueKey, ValueMap, ValueVec,
    },
    vm::{CallArgs, KotoVm, KotoVmSettings, ModuleImportedCallback, ReturnOrYield},
};

/// The shared API backend marker for the runtime.
pub type Backend = RuntimeBackend;

pub use koto_api as api;
pub use koto_derive as derive;
pub use koto_memory::{Borrow, BorrowMut, KCell, Ptr, PtrMut, lazy, make_ptr, make_ptr_mut};

#[doc(hidden)]
pub mod __private;
