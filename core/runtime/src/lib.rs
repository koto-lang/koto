//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

mod display_context;
mod error;
mod file;
mod frame;
mod stdio;
mod types;
mod vm;

pub mod core_lib;
pub mod prelude;

pub use crate::{
    display_context::DisplayContext,
    error::{type_error, type_error_with_slice, Result, RuntimeError},
    file::{KotoFile, KotoRead, KotoWrite},
    stdio::{DefaultStderr, DefaultStdin, DefaultStdout},
    types::{
        ArgRegisters, BinaryOp, DataMap, ExternalFunction, FunctionInfo, IntRange, IsIterable,
        KotoHasher, KotoIterator, KotoObject, KotoType, MetaKey, MetaMap, MethodContext, Object,
        ObjectEntryBuilder, SimpleFunctionInfo, UnaryOp, Value, ValueIterator, ValueIteratorOutput,
        ValueKey, ValueList, ValueMap, ValueNumber, ValueString, ValueTuple, ValueVec,
    },
    vm::{CallArgs, ModuleImportedCallback, Vm, VmSettings},
};
pub use koto_memory::{Borrow, BorrowMut, Ptr, PtrMut};
