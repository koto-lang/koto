mod callable;
mod function;
mod iterator;
mod list;
pub(crate) mod map;
mod native_function;
mod number;
mod object;
mod range;
mod scalars;
mod string;
mod tuple;
mod value;

pub use function::KFunction;
pub use iterator::{KIterator, KIteratorOutput};
pub use list::KList;
pub use map::{KMap, MetaKey};
pub use native_function::KNativeFunction;
pub use number::KNumber;
pub use object::{
    Borrow, BorrowMut, IsIterable, KObject, KotoField, KotoObject, MethodContext, ObjectBorrow,
    ObjectBorrowMut, PluginBackend,
};
pub use range::KRange;
pub use string::KString;
pub use tuple::KTuple;
pub use value::KValue;

use crate::abi;
use crate::{Result, runtime_error};
#[cfg(target_arch = "wasm32")]
use koto_ffi::wasm;

pub(crate) use object::make_method_value;

pub(crate) fn decode_value(api: &abi::KotoHostApiV1, value: abi::KValue) -> Result<KValue> {
    match value.kind {
        abi::KValueKind::Null => Ok(KValue::Null),
        abi::KValueKind::Bool => Ok(KValue::Bool(unsafe { value.data.bool_value })),
        abi::KValueKind::I64 => Ok(KValue::Number(KNumber::I64(unsafe {
            value.data.i64_value
        }))),
        abi::KValueKind::F64 => Ok(KValue::Number(KNumber::F64(unsafe {
            value.data.f64_value
        }))),
        abi::KValueKind::Range => Ok(KValue::Range(unsafe { value.data.range_value }.into())),
        abi::KValueKind::String => Ok(KValue::Str(KString::from_existing(api, value))),
        abi::KValueKind::List => Ok(KValue::List(KList::from_existing(api, value))),
        abi::KValueKind::Tuple => Ok(KValue::Tuple(KTuple::from_existing(api, value))),
        abi::KValueKind::Map => Ok(KValue::Map(KMap::from_existing(api, value))),
        abi::KValueKind::Function => Ok(KValue::Function(KFunction::from_existing(api, value))),
        abi::KValueKind::NativeFunction => Ok(KValue::NativeFunction(
            KNativeFunction::from_existing(api, value),
        )),
        abi::KValueKind::Iterator => Ok(KValue::Iterator(iterator::KIterator::from_existing(
            api, value,
        ))),
        abi::KValueKind::Object => Ok(KValue::Object(object::KObject::from_existing(api, value))),
        abi::KValueKind::Unsupported => {
            runtime_error!("unsupported runtime value for plugin ABI v1")
        }
    }
}

pub(crate) fn encode_value(_api: &abi::KotoHostApiV1, value: KValue) -> abi::KValue {
    match value {
        KValue::Null => abi::KValue::null(),
        KValue::Bool(value) => abi::KValue {
            kind: abi::KValueKind::Bool,
            data: abi::KValueData { bool_value: value },
        },
        KValue::Number(KNumber::I64(value)) => abi::KValue {
            kind: abi::KValueKind::I64,
            data: abi::KValueData { i64_value: value },
        },
        KValue::Number(KNumber::F64(value)) => abi::KValue {
            kind: abi::KValueKind::F64,
            data: abi::KValueData { f64_value: value },
        },
        KValue::Range(value) => abi::KValue {
            kind: abi::KValueKind::Range,
            data: abi::KValueData {
                range_value: value.into(),
            },
        },
        KValue::Str(value) => value.into_raw(),
        KValue::List(value) => value.into_raw(),
        KValue::Tuple(value) => value.into_raw(),
        KValue::Map(value) => value.into_raw(),
        KValue::Function(value) => value.into_raw(),
        KValue::NativeFunction(value) => value.into_raw(),
        KValue::Iterator(value) => value.into_raw(),
        KValue::Object(value) => value.into_raw(),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn decode_wasm_value(value: wasm::KValue) -> Result<KValue> {
    let value = crate::wasm_support::wasm_value_to_native(value);
    match value.kind {
        abi::KValueKind::Null => Ok(KValue::Null),
        abi::KValueKind::Bool => Ok(KValue::Bool(unsafe { value.data.bool_value })),
        abi::KValueKind::I64 => Ok(KValue::Number(KNumber::I64(unsafe {
            value.data.i64_value
        }))),
        abi::KValueKind::F64 => Ok(KValue::Number(KNumber::F64(unsafe {
            value.data.f64_value
        }))),
        abi::KValueKind::Range => Ok(KValue::Range(unsafe { value.data.range_value }.into())),
        abi::KValueKind::String => Ok(KValue::Str(KString::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::List => Ok(KValue::List(KList::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::Tuple => Ok(KValue::Tuple(KTuple::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::Map => Ok(KValue::Map(KMap::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::Object => Ok(KValue::Object(KObject::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::Iterator => Ok(KValue::Iterator(iterator::KIterator::from_wasm_existing(
            crate::wasm_support::native_value_to_wasm(value),
        ))),
        abi::KValueKind::Function
        | abi::KValueKind::NativeFunction
        | abi::KValueKind::Unsupported => {
            runtime_error!("unsupported wasm value for plugin callback decoding")
        }
    }
}
