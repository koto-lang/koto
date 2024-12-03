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
    function::{KCaptureFunction, KFunction},
    iterator::{KIterator, KIteratorOutput, KotoIterator},
    list::{KList, ValueVec},
    map::{KMap, KotoHasher, ValueMap},
    meta_map::{meta_id_to_key, BinaryOp, MetaKey, MetaMap, UnaryOp},
    native_function::{CallContext, KNativeFunction, KotoFunction},
    number::KNumber,
    object::{
        IsIterable, KObject, KotoCopy, KotoEntries, KotoField, KotoObject, KotoType, MethodContext,
    },
    range::KRange,
    tuple::KTuple,
    value::KValue,
    value_key::ValueKey,
};
