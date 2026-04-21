use crate::{Error, KRange, KValue, Result};
use koto_ffi::{KValueKind, KotoStatusCode, MetaKeyKind, wasm};
use wasmi::{Caller, Memory};

use super::{
    handles::{
        GuestResource, HostState, WasmHandle, call_guest_drop_with_caller, insert_wasm_handle,
        release_guest_resource_count,
    },
    imports::get_memory,
    objects::{
        WasmIteratorData, decode_wasm_native_function, encode_runtime_native_function,
        make_wasm_object,
    },
    runtime::WasmRuntime,
};

pub(super) fn encode_runtime_value(state: &mut HostState, value: KValue) -> Result<wasm::KValue> {
    Ok(match value {
        KValue::Null => wasm::KValue::null(),
        KValue::Bool(value) => wasm::KValue {
            kind: KValueKind::Bool,
            data: wasm::KValueData { bool_value: value },
        },
        KValue::Number(number) => match number {
            crate::KNumber::I64(value) => wasm::KValue {
                kind: KValueKind::I64,
                data: wasm::KValueData { i64_value: value },
            },
            crate::KNumber::F64(value) => wasm::KValue {
                kind: KValueKind::F64,
                data: wasm::KValueData { f64_value: value },
            },
        },
        KValue::Range(range) => wasm::KValue {
            kind: KValueKind::Range,
            data: wasm::KValueData {
                range_value: wasm::KotoRange {
                    start: range.start().unwrap_or_default(),
                    has_start: range.start().is_some(),
                    end: range.end().map(|(end, _)| end).unwrap_or_default(),
                    has_end: range.end().is_some(),
                    end_inclusive: range.end().map(|(_, inclusive)| inclusive).unwrap_or(false),
                },
            },
        },
        KValue::Str(value) => {
            let handle = insert_wasm_handle(state, WasmHandle::String(value));
            wasm::KValue {
                kind: KValueKind::String,
                data: wasm::KValueData {
                    string_value: wasm::KString {
                        kind: wasm::KStringKind::Full,
                        data: wasm::KStringData { full: handle },
                    },
                },
            }
        }
        KValue::Tuple(value) => {
            let handle = insert_wasm_handle(state, WasmHandle::Tuple(value));
            wasm::KValue {
                kind: KValueKind::Tuple,
                data: wasm::KValueData {
                    tuple_value: wasm::KTuple {
                        kind: wasm::KTupleKind::Full,
                        data: wasm::KTupleData { full: handle },
                    },
                },
            }
        }
        KValue::List(value) => {
            let handle = insert_wasm_handle(state, WasmHandle::List(value));
            wasm::KValue {
                kind: KValueKind::List,
                data: wasm::KValueData { handle },
            }
        }
        KValue::Map(value) => {
            let handle = insert_wasm_handle(state, WasmHandle::Map(value));
            wasm::KValue {
                kind: KValueKind::Map,
                data: wasm::KValueData {
                    map_value: wasm::KMap {
                        data: handle,
                        meta: 0,
                    },
                },
            }
        }
        KValue::Object(object) => {
            let Some(data) = object.try_borrow().ok().and_then(|object| {
                (&*object as &dyn std::any::Any)
                    .downcast_ref::<super::objects::WasmObjectData>()
                    .cloned()
            }) else {
                return Err(Error::from(
                    "passing non-wasm objects to wasm plugins isn't supported yet",
                ));
            };

            let handle = insert_wasm_handle(state, WasmHandle::Object(data.user_data));
            wasm::KValue {
                kind: KValueKind::Object,
                data: wasm::KValueData {
                    object_value: wasm::KObject {
                        data: handle,
                        metadata: data.user_data,
                    },
                },
            }
        }
        KValue::NativeFunction(function) => {
            let Some(registered) = encode_runtime_native_function(&function) else {
                return Err(Error::from(
                    "passing non-wasm runtime native functions to wasm plugins isn't supported yet",
                ));
            };
            let handle = insert_wasm_handle(state, WasmHandle::NativeFunction(registered));
            wasm::KValue {
                kind: KValueKind::NativeFunction,
                data: wasm::KValueData {
                    native_function_value: wasm::OpaqueHandle {
                        data: handle,
                        metadata: 0,
                    },
                },
            }
        }
        KValue::Function(_) | KValue::Iterator(_) | KValue::TemporaryTuple(_) => {
            return Err(Error::from(format!(
                "unsupported runtime value for wasm plugin host: {}",
                value.type_as_string()
            )));
        }
    })
}

pub(super) fn decode_wasm_value(state: &mut HostState, value: wasm::KValue) -> Result<KValue> {
    match value.kind {
        KValueKind::Null => Ok(KValue::Null),
        KValueKind::Bool => Ok(KValue::Bool(unsafe { value.data.bool_value })),
        KValueKind::I64 => Ok(unsafe { value.data.i64_value }.into()),
        KValueKind::F64 => Ok(unsafe { value.data.f64_value }.into()),
        KValueKind::Range => {
            let range = unsafe { value.data.range_value };
            Ok(KRange::new(
                range.has_start.then_some(range.start),
                range.has_end.then_some((range.end, range.end_inclusive)),
            )
            .into())
        }
        KValueKind::String => {
            let handle = unsafe { value.data.string_value.data.full };
            let Some(WasmHandle::String(value)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm string handle"));
            };
            Ok(KValue::Str(value.clone()))
        }
        KValueKind::Tuple => {
            let handle = unsafe { value.data.tuple_value.data.full };
            let Some(WasmHandle::Tuple(value)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm tuple handle"));
            };
            Ok(KValue::Tuple(value.clone()))
        }
        KValueKind::List => {
            let handle = unsafe { value.data.handle };
            let Some(WasmHandle::List(value)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm list handle"));
            };
            Ok(KValue::List(value.clone()))
        }
        KValueKind::Map => {
            let handle = unsafe { value.data.map_value.data };
            let Some(WasmHandle::Map(value)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm map handle"));
            };
            Ok(KValue::Map(value.clone()))
        }
        KValueKind::Object => {
            let object = unsafe { value.data.object_value };
            let handle = object.data;
            let user_data = object.metadata;
            let Some(WasmHandle::Object(_)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm object handle"));
            };
            *state
                .guest_resources
                .entry(GuestResource::Object(user_data))
                .or_default() += 1;
            let runtime = state
                .runtime_target
                .lock()
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| Error::from("wasm module runtime is no longer available"))?;
            Ok(KValue::Object(make_wasm_object(handle, user_data, runtime)))
        }
        KValueKind::Iterator => {
            let iterator = unsafe { value.data.iterator_value };
            let handle = iterator.data;
            let user_data = iterator.metadata;
            let Some(WasmHandle::Iterator(_)) = state.handles.get(handle) else {
                return Err(Error::from("invalid wasm iterator handle"));
            };
            *state
                .guest_resources
                .entry(GuestResource::Iterator(user_data))
                .or_default() += 1;
            let runtime = state
                .runtime_target
                .lock()
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| Error::from("wasm module runtime is no longer available"))?;
            Ok(KValue::Iterator(crate::KIterator::new(WasmIteratorData {
                user_data,
                runtime,
            })))
        }
        KValueKind::NativeFunction => {
            let handle = unsafe { value.data.native_function_value.data };
            let Some(WasmHandle::NativeFunction(registered)) = state.handles.get(handle).cloned()
            else {
                return Err(Error::from("invalid wasm native function handle"));
            };
            *state
                .guest_resources
                .entry(GuestResource::NativeFunction(registered.user_data))
                .or_default() += 1;
            let runtime = state
                .runtime_target
                .lock()
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| Error::from("wasm module runtime is no longer available"))?;
            Ok(decode_wasm_native_function(registered, runtime))
        }
        _ => Err(Error::from("unsupported wasm value kind")),
    }
}

pub(super) fn clone_wasm_value(state: &mut HostState, value: wasm::KValue) -> Result<wasm::KValue> {
    if value.kind == KValueKind::Object {
        let object = unsafe { value.data.object_value };
        let handle = object.data;
        let Some(WasmHandle::Object(user_data)) = state.handles.get(handle) else {
            return Err(Error::from("invalid wasm object handle"));
        };
        let cloned_handle = insert_wasm_handle(state, WasmHandle::Object(*user_data));
        return Ok(wasm::KValue {
            kind: KValueKind::Object,
            data: wasm::KValueData {
                object_value: wasm::KObject {
                    data: cloned_handle,
                    metadata: object.metadata,
                },
            },
        });
    }
    if value.kind == KValueKind::Iterator {
        let iterator = unsafe { value.data.iterator_value };
        let handle = iterator.data;
        let Some(WasmHandle::Iterator(user_data)) = state.handles.get(handle) else {
            return Err(Error::from("invalid wasm iterator handle"));
        };
        let cloned_handle = insert_wasm_handle(state, WasmHandle::Iterator(*user_data));
        return Ok(wasm::KValue {
            kind: KValueKind::Iterator,
            data: wasm::KValueData {
                iterator_value: wasm::OpaqueHandle {
                    data: cloned_handle,
                    metadata: iterator.metadata,
                },
            },
        });
    }

    let decoded = decode_wasm_value(state, value)?;
    encode_runtime_value(state, decoded)
}

pub(super) fn drop_wasm_value(runtime: &mut WasmRuntime, value: wasm::KValue) {
    let handle = match value.kind {
        KValueKind::String => Some(unsafe { value.data.string_value.data.full }),
        KValueKind::Tuple => Some(unsafe { value.data.tuple_value.data.full }),
        KValueKind::List => Some(unsafe { value.data.handle }),
        KValueKind::Map => Some(unsafe { value.data.map_value.data }),
        KValueKind::Object => Some(unsafe { value.data.object_value.data }),
        KValueKind::Iterator => Some(unsafe { value.data.iterator_value.data }),
        KValueKind::NativeFunction => Some(unsafe { value.data.native_function_value.data }),
        _ => None,
    };

    if let Some(handle) = handle {
        let removed = runtime.store.data_mut().handles.remove(handle);

        if let Some(resource) = removed.as_ref().and_then(WasmHandle::guest_resource) {
            runtime.release_guest_resource(resource);
        }
    }
}

pub(super) fn drop_wasm_value_with_caller(
    caller: &mut Caller<'_, HostState>,
    value: wasm::KValue,
) -> std::result::Result<(), wasmi::Error> {
    let handle = match value.kind {
        KValueKind::String => Some(unsafe { value.data.string_value.data.full }),
        KValueKind::Tuple => Some(unsafe { value.data.tuple_value.data.full }),
        KValueKind::List => Some(unsafe { value.data.handle }),
        KValueKind::Map => Some(unsafe { value.data.map_value.data }),
        KValueKind::Object => Some(unsafe { value.data.object_value.data }),
        KValueKind::Iterator => Some(unsafe { value.data.iterator_value.data }),
        KValueKind::NativeFunction => Some(unsafe { value.data.native_function_value.data }),
        _ => None,
    };

    if let Some(handle) = handle {
        let removed = caller.data_mut().handles.remove(handle);

        if let Some(resource) = removed.as_ref().and_then(WasmHandle::guest_resource) {
            let should_drop = release_guest_resource_count(caller.data_mut(), &resource);
            if should_drop {
                call_guest_drop_with_caller(caller, &resource)?;
            }
        }
    }

    Ok(())
}

pub(super) fn decode_meta_key(
    caller: &mut Caller<'_, HostState>,
    key: wasm::MetaKey,
) -> Result<Option<crate::MetaKey>> {
    Ok(Some(match key.kind {
        MetaKeyKind::UnaryOp => crate::MetaKey::UnaryOp(unsafe { key.data.unary_op }),
        MetaKeyKind::BinaryOp => crate::MetaKey::BinaryOp(unsafe { key.data.binary_op }),
        MetaKeyKind::ReadOp => crate::MetaKey::ReadOp(unsafe { key.data.read_op }),
        MetaKeyKind::WriteOp => crate::MetaKey::WriteOp(unsafe { key.data.write_op }),
        MetaKeyKind::Call => crate::MetaKey::Call,
        MetaKeyKind::Named => {
            let slice = unsafe { key.data.string };
            let memory = get_memory(&*caller).map_err(wasmi_error_to_runtime)?;
            let mut bytes = vec![0; slice.len as usize];
            memory
                .read(caller, slice.ptr as usize, &mut bytes)
                .map_err(|error| Error::from(error.to_string()))?;
            let name = String::from_utf8(bytes).map_err(|error| Error::from(error.to_string()))?;
            crate::MetaKey::Named(name.into())
        }
        MetaKeyKind::Type => crate::MetaKey::Type,
        MetaKeyKind::Base => crate::MetaKey::Base,
        _ => return Ok(None),
    }))
}

pub(super) fn status_to_runtime_error(
    memory: &Memory,
    store: &wasmi::Store<HostState>,
    status: wasm::KotoStatus,
) -> Error {
    if status.message.len > 0 {
        let mut bytes = vec![0; status.message.len as usize];
        if memory
            .read(store, status.message.ptr as usize, &mut bytes)
            .is_ok()
            && let Ok(message) = String::from_utf8(bytes)
        {
            return Error::from(message);
        }
    }

    Error::from("wasm plugin operation failed")
}

pub(super) fn runtime_error_to_status(error: Error) -> wasm::KotoStatus {
    wasm::KotoStatus {
        code: KotoStatusCode::Error,
        error: 0,
        is_unimplemented: error.is_unimplemented_error(),
        message: wasm::KStringSlice { ptr: 0, len: 0 },
    }
}

pub(super) fn runtime_error_to_wasmi(error: Error) -> wasmi::Error {
    wasmi::Error::new(error.to_string())
}

pub(super) fn wasmi_error_to_runtime(error: wasmi::Error) -> Error {
    Error::from(error.to_string())
}
