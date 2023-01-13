//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

mod display;
mod error;
mod external;
mod file;
mod frame;
mod meta_map;
mod stdio;
mod value_iterator;
mod value_key;
mod value_list;
mod value_map;
mod value_number;
mod value_sort;
mod value_string;
mod value_tuple;
mod vm;

pub mod core;
pub mod prelude;
pub mod value;

pub use {
    display::{KotoDisplay, KotoDisplayOptions},
    error::{type_error, type_error_with_slice, RuntimeError, RuntimeResult},
    external::{ExternalData, ExternalFunction, ExternalValue},
    file::{KotoFile, KotoRead, KotoWrite},
    meta_map::{BinaryOp, MetaKey, MetaMap, MetaMapBuilder, UnaryOp},
    stdio::{DefaultStderr, DefaultStdin, DefaultStdout},
    value::{FunctionInfo, IntRange, Value},
    value_iterator::{KotoIterator, ValueIterator, ValueIteratorOutput},
    value_key::ValueKey,
    value_list::{ValueList, ValueVec},
    value_map::{DataMap, ValueMap},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::{CallArgs, ModuleImportedCallback, Vm, VmSettings},
};
