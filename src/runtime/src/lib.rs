//! Contains the runtime and core library for the Koto language

pub mod core;
mod error;
mod external;
mod frame;
mod logger;
mod meta_map;
pub mod num2;
pub mod num4;
pub mod value;
mod value_iterator;
mod value_key;
mod value_list;
mod value_map;
mod value_number;
mod value_sort;
mod value_string;
mod value_tuple;
mod vm;

pub use {
    error::*,
    external::{is_external_instance, visit_external_value, ExternalFunction, ExternalValue},
    koto_bytecode::{CompilerError, Loader, LoaderError},
    koto_parser::ParserError,
    logger::{DefaultLogger, KotoLogger},
    meta_map::{BinaryOp, MetaKey, MetaMap, UnaryOp},
    num2::Num2,
    num4::Num4,
    value::{RuntimeFunction, Value},
    value_iterator::{IntRange, ValueIterator, ValueIteratorOutput},
    value_key::{ValueKey, ValueRef},
    value_list::{ValueList, ValueVec},
    value_map::{ValueHashMap, ValueMap},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::{Vm, VmSettings},
};
