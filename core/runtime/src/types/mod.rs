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
    external_function::{CallContext, KNativeFunction},
    int_range::KRange,
    meta_map::{meta_id_to_key, BinaryOp, MetaKey, MetaMap, UnaryOp},
    object::{IsIterable, KObject, KotoObject, KotoType, MethodContext, ObjectEntryBuilder},
    value::{KCaptureFunction, KFunction, Value},
    value_iterator::{KIterator, KIteratorOutput, KotoIterator},
    value_key::ValueKey,
    value_list::{KList, ValueVec},
    value_map::{KMap, KotoHasher, ValueMap},
    value_number::KNumber,
    value_string::KString,
    value_tuple::KTuple,
};
