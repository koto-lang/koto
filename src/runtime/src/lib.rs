pub mod core;
mod error;
mod external;
mod frame;
mod loader;
pub mod num2;
pub mod num4;
pub mod value;
mod value_iterator;
mod value_list;
mod value_map;
mod value_string;
mod value_tuple;
mod vm;

pub use {
    error::*,
    external::{visit_external_value, ExternalFunction, ExternalValue},
    koto_bytecode::CompilerError,
    koto_parser::ParserError,
    loader::{Loader, LoaderError},
    num2::Num2,
    num4::Num4,
    value::{
        make_external_value, type_as_string, value_is_immutable, RuntimeFunction, Value, ValueRef,
    },
    value_iterator::{IntRange, ValueIterator},
    value_list::{ValueList, ValueVec},
    value_map::{ValueHashMap, ValueMap, ValueMapKey},
    value_string::ValueString,
    value_tuple::ValueTuple,
    vm::{Vm, VmContext},
};
