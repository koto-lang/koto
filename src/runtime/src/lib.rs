//! Contains the runtime and core library for the Koto language

pub mod core;
mod error;
mod external;
mod frame;
mod logger;
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
    num2::Num2,
    num4::Num4,
    value::{make_external_value, type_as_string, RuntimeFunction, Value},
    value_iterator::{IntRange, ValueIterator, ValueIteratorOutput},
    value_key::{value_is_immutable, ValueKey},
    value_list::{ValueList, ValueVec},
    value_map::{BinaryOp, MetaKey, UnaryOp, ValueHashMap, ValueMap},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::{Vm, VmSettings},
};
