//! The core types used in the Koto runtime

mod external_function;
mod int_range;
mod meta_map;
mod object;
pub mod value;
mod value_iterator;
mod value_key;
mod value_list;
mod value_map;
mod value_number;
mod value_string;
mod value_tuple;

pub use self::{
    external_function::{CallContext, ExternalFunction},
    int_range::IntRange,
    meta_map::{meta_id_to_key, BinaryOp, MetaKey, MetaMap, UnaryOp},
    object::{IsIterable, KotoObject, KotoType, MethodContext, Object, ObjectEntryBuilder},
    value::{FunctionInfo, SimpleFunctionInfo, Value},
    value_iterator::{KotoIterator, ValueIterator, ValueIteratorOutput},
    value_key::ValueKey,
    value_list::{ValueList, ValueVec},
    value_map::{DataMap, KotoHasher, ValueMap},
    value_number::ValueNumber,
    value_string::ValueString,
    value_tuple::ValueTuple,
};
