//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

mod display_context;
mod error;
mod io;
mod types;
mod vm;

pub mod core_lib;
pub mod prelude;

pub use crate::{
    display_context::DisplayContext,
    error::{type_error, type_error_with_slice, Error, Result},
    io::{BufferedFile, DefaultStderr, DefaultStdin, DefaultStdout, KotoFile, KotoRead, KotoWrite},
    types::{
        BinaryOp, CallContext, IsIterable, KCaptureFunction, KFunction, KIterator, KIteratorOutput,
        KList, KMap, KNativeFunction, KNumber, KObject, KRange, KString, KTuple, KotoCopy,
        KotoHasher, KotoIterator, KotoLookup, KotoObject, KotoType, MetaKey, MetaMap,
        MethodContext, UnaryOp, Value, ValueKey, ValueMap, ValueVec,
    },
    vm::{CallArgs, ModuleImportedCallback, Vm, VmSettings},
};
pub use koto_derive as derive;
pub use koto_memory::{Borrow, BorrowMut, Ptr, PtrMut};
