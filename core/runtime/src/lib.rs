//! Contains the runtime and core library for the Koto language

#![warn(missing_docs)]

mod display;
mod error;
mod external_function;
mod external_value;
mod file;
mod frame;
mod int_range;
mod meta_map;
mod meta_map_builder;
mod rc_cell;
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
    external_function::ExternalFunction,
    external_value::{External, ExternalData},
    file::{KotoFile, KotoRead, KotoWrite},
    int_range::IntRange,
    meta_map::{BinaryOp, MetaKey, MetaMap, UnaryOp},
    meta_map_builder::MetaMapBuilder,
    rc_cell::RcCell,
    stdio::{DefaultStderr, DefaultStdin, DefaultStdout},
    value::{FunctionInfo, Value},
    value_iterator::{KotoIterator, ValueIterator, ValueIteratorOutput},
    value_key::ValueKey,
    value_list::{ValueList, ValueVec},
    value_map::{DataMap, KotoHasher, ValueMap},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::{CallArgs, ModuleImportedCallback, Vm, VmSettings},
};
