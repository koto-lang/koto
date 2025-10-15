//! The core types used in the Koto runtime

mod function;
mod iterator;
mod list;
mod map;
mod meta_map;
mod native_function;
mod number;
mod object;
mod range;
mod tuple;
pub mod value;
mod value_key;

pub use koto_parser::KString;

pub use self::{
    function::{FunctionContext, KFunction},
    iterator::{KIterator, KIteratorOutput, KotoIterator},
    list::{KList, ValueVec},
    map::{KMap, KotoHasher, ValueMap},
    meta_map::{BinaryOp, MetaKey, MetaMap, ReadOp, UnaryOp, WriteOp, meta_id_to_key},
    native_function::{CallContext, KNativeFunction, KotoFunction},
    number::KNumber,
    object::{
        IsIterable, KObject, KotoAccess, KotoCopy, KotoField, KotoObject, KotoType, MethodContext,
    },
    range::KRange,
    tuple::KTuple,
    value::KValue,
    value_key::ValueKey,
};
