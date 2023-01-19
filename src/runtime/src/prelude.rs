//! A collection of useful items to make it easier to work with `koto_runtime`

#[doc(inline)]
pub use crate::{
    make_runtime_error, runtime_error, type_error, type_error_with_slice, BinaryOp, CallArgs,
    DataMap, ExternalData, ExternalValue, IntRange, KotoDisplay, KotoDisplayOptions, KotoFile,
    KotoHasher, KotoIterator, KotoRead, KotoWrite, MetaKey, MetaMap, MetaMapBuilder, RuntimeError,
    RuntimeResult, UnaryOp, Value, ValueIterator, ValueIteratorOutput, ValueKey, ValueList,
    ValueMap, ValueNumber, ValueString, ValueTuple, ValueVec, Vm, VmSettings,
};
