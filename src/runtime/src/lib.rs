//! Contains the runtime and core library for the Koto language

pub mod core;
mod error;
mod external;
mod frame;
pub mod num2;
pub mod num4;
pub mod value;
mod value_iterator;
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
    num2::Num2,
    num4::Num4,
    value::{
        make_external_value, type_as_string, value_is_immutable, RuntimeFunction, Value, ValueRef,
    },
    value_iterator::{IntRange, ValueIterator, ValueIteratorOutput},
    value_list::{ValueList, ValueVec},
    value_map::{operator_as_string, MetaKey, Operator, ValueHashMap, ValueMap, ValueMapKey},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::Vm,
};
