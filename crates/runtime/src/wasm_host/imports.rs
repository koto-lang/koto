use crate::{Error, KValue};
use koto_api::KotoCollection;
use koto_ffi::{KValueKind, wasm};
use std::{
    mem::{MaybeUninit, align_of, size_of},
    ptr, slice,
};
use wasmi::{Caller, Extern, Linker, Memory};

use super::{
    codec::{
        clone_wasm_value, decode_meta_key, decode_wasm_value, drop_wasm_value_with_caller,
        encode_runtime_value, runtime_error_to_status, runtime_error_to_wasmi,
        wasmi_error_to_runtime,
    },
    handles::{HostState, RegisteredFunction, WasmHandle, insert_wasm_handle},
};

pub(super) fn define_host_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    define_value_imports(linker)?;
    define_sequence_imports(linker)?;
    define_map_imports(linker)?;
    define_callable_imports(linker)?;
    define_object_imports(linker)?;
    Ok(())
}

fn define_value_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker
        .func_wrap(
            "koto",
            "koto_value_make_i64",
            |mut caller: Caller<'_, HostState>, value: i64, out_ptr: i32| {
                let value = wasm::KValue {
                    kind: KValueKind::I64,
                    data: wasm::KValueData { i64_value: value },
                };
                write_guest_struct(&mut caller, out_ptr as u32, &value)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_string_make",
            |mut caller: Caller<'_, HostState>, value_ptr: i32, out_ptr: i32| {
                let string = read_guest_string(&mut caller, value_ptr as u32)?;
                let handle = caller
                    .data_mut()
                    .handles
                    .insert(WasmHandle::String(crate::KString::from(string)));
                let value = wasm::KString {
                    kind: wasm::KStringKind::Full,
                    data: wasm::KStringData { full: handle },
                };
                write_guest_struct(&mut caller, out_ptr as u32, &value)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_string_as_slice",
            |mut caller: Caller<'_, HostState>, string_ptr: i32, out_ptr: i32| {
                let string = read_guest_struct::<wasm::KString>(&caller, string_ptr as u32)?;
                let handle = unsafe { string.data.full };
                let Some(WasmHandle::String(value)) = caller.data().handles.get(handle).cloned()
                else {
                    return Err(wasmi::Error::new("invalid wasm string handle"));
                };
                let bytes = value.as_str().as_bytes().to_vec();
                let guest_ptr = guest_alloc(&mut caller, bytes.len() as u32, 1)?;
                write_guest_bytes(&mut caller, guest_ptr, &bytes)?;
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KStringSlice {
                        ptr: guest_ptr,
                        len: bytes.len() as u32,
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_string_slice_free",
            |mut caller: Caller<'_, HostState>, string_ptr: i32| {
                let slice = read_guest_struct::<wasm::KStringSlice>(&caller, string_ptr as u32)?;
                if slice.ptr != 0 && slice.len != 0 {
                    guest_free(&mut caller, slice.ptr, slice.len as usize, 1)?;
                }
                Ok(())
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_value_clone",
            |mut caller: Caller<'_, HostState>, value_ptr: i32, out_ptr: i32| {
                let value = read_guest_struct::<wasm::KValue>(&caller, value_ptr as u32)?;
                let cloned =
                    clone_wasm_value(caller.data_mut(), value).map_err(runtime_error_to_wasmi)?;
                write_guest_struct(&mut caller, out_ptr as u32, &cloned)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_value_free",
            |mut caller: Caller<'_, HostState>, value_ptr: i32| {
                let value = read_guest_struct::<wasm::KValue>(&caller, value_ptr as u32)?;
                drop_wasm_value_with_caller(&mut caller, value)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_value_view_clone",
            |mut caller: Caller<'_, HostState>, value_view_ptr: i32, out_ptr: i32| {
                let view = read_guest_struct::<wasm::KValueView>(&caller, value_view_ptr as u32)?;
                let handle = view.0;
                let Some(WasmHandle::ValueView(value)) = caller.data().handles.get(handle).cloned()
                else {
                    return Err(wasmi::Error::new("invalid wasm value view handle"));
                };
                let encoded = encode_runtime_value(caller.data_mut(), value)
                    .map_err(runtime_error_to_wasmi)?;
                write_guest_struct(&mut caller, out_ptr as u32, &encoded)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_value_view_free",
            |mut caller: Caller<'_, HostState>, value_view_ptr: i32| {
                let view = read_guest_struct::<wasm::KValueView>(&caller, value_view_ptr as u32)?;
                let _ = caller.data_mut().handles.remove(view.0);
                Ok(())
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(())
}

fn define_sequence_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker
        .func_wrap(
            "koto",
            "koto_tuple_make",
            |mut caller: Caller<'_, HostState>, values_ptr: i32, len: i32, out_ptr: i32| {
                let values = read_guest_values(&caller, values_ptr as u32, len as usize)?;
                let values = values
                    .into_iter()
                    .map(|value| decode_wasm_value(caller.data_mut(), value))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(runtime_error_to_wasmi)?;
                let handle = insert_wasm_handle(
                    caller.data_mut(),
                    WasmHandle::Tuple(crate::KTuple::from(values)),
                );
                let value = wasm::KTuple {
                    kind: wasm::KTupleKind::Full,
                    data: wasm::KTupleData { full: handle },
                };
                write_guest_struct(&mut caller, out_ptr as u32, &value)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_tuple_len",
            |caller: Caller<'_, HostState>, tuple_ptr: i32| -> Result<i32, wasmi::Error> {
                Ok(read_guest_tuple(&caller, tuple_ptr as u32)?.len() as i32)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_tuple_data",
            |mut caller: Caller<'_, HostState>, tuple_ptr: i32, out_ptr: i32| {
                let value = read_guest_tuple(&caller, tuple_ptr as u32)?;
                write_guest_value_slice(
                    &mut caller,
                    value.iter().cloned().collect(),
                    out_ptr as u32,
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_value_slice_free",
            |mut caller: Caller<'_, HostState>, slice_ptr: i32| {
                let slice = read_guest_struct::<wasm::KValueSlice>(&caller, slice_ptr as u32)?;
                if slice.data != 0 && slice.len != 0 {
                    let view_count = slice.len as usize;
                    let views = read_guest_value_views(&caller, slice.data, view_count)?;
                    for view in views {
                        let _ = caller.data_mut().handles.remove(view.0);
                    }
                    guest_free(
                        &mut caller,
                        slice.data,
                        slice.stride as usize * view_count,
                        align_of::<wasm::KValueView>(),
                    )?;
                }
                Ok(())
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_list_make",
            |mut caller: Caller<'_, HostState>, values_ptr: i32, len: i32, out_ptr: i32| {
                let values = read_guest_values(&caller, values_ptr as u32, len as usize)?;
                let values = values
                    .into_iter()
                    .map(|value| decode_wasm_value(caller.data_mut(), value))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(runtime_error_to_wasmi)?;
                let handle = insert_wasm_handle(
                    caller.data_mut(),
                    WasmHandle::List(crate::KList::from(values)),
                );
                write_guest_struct(&mut caller, out_ptr as u32, &wasm::KList(handle))
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_list_len",
            |caller: Caller<'_, HostState>, list_ptr: i32| -> Result<i32, wasmi::Error> {
                Ok(read_guest_list(&caller, list_ptr as u32)?.len() as i32)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_list_data",
            |mut caller: Caller<'_, HostState>, list_ptr: i32, out_ptr: i32| {
                let value = read_guest_list(&caller, list_ptr as u32)?;
                write_guest_value_slice(
                    &mut caller,
                    value.data().iter().cloned().collect(),
                    out_ptr as u32,
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(())
}

fn define_map_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker
        .func_wrap(
            "koto",
            "koto_map_make",
            |mut caller: Caller<'_, HostState>, entries_ptr: i32, len: i32, out_ptr: i32| {
                let entries = read_guest_entries(&caller, entries_ptr as u32, len as usize)?;
                let entries = entries
                    .into_iter()
                    .map(|entry| {
                        let key = decode_wasm_value(caller.data_mut(), entry.key)?;
                        let value = decode_wasm_value(caller.data_mut(), entry.value)?;
                        Ok((key, value))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(runtime_error_to_wasmi)?;
                let map = crate::KMap::from_entries(&entries);
                let handle = insert_wasm_handle(caller.data_mut(), WasmHandle::Map(map));
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KMap {
                        data: handle,
                        meta: 0,
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_new_with_type",
            |mut caller: Caller<'_, HostState>, type_name_ptr: i32, out_ptr: i32| {
                let type_name = read_guest_string(&mut caller, type_name_ptr as u32)?;
                let handle = insert_wasm_handle(
                    caller.data_mut(),
                    WasmHandle::Map(crate::KMap::with_type(&type_name)),
                );
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KMap {
                        data: handle,
                        meta: 0,
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_len",
            |caller: Caller<'_, HostState>, map_ptr: i32| -> Result<i32, wasmi::Error> {
                Ok(read_guest_map(&caller, map_ptr as u32)?.len() as i32)
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_data",
            |mut caller: Caller<'_, HostState>, map_ptr: i32, out_ptr: i32| {
                let value = read_guest_map(&caller, map_ptr as u32)?;
                write_guest_map_data(
                    &mut caller,
                    value
                        .data()
                        .iter()
                        .map(|(key, value)| (KValue::from(key.clone()), value.clone()))
                        .collect::<Vec<_>>(),
                    out_ptr as u32,
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_data_free",
            |mut caller: Caller<'_, HostState>, map_data_ptr: i32| {
                let map_data = read_guest_struct::<wasm::KMapData>(&caller, map_data_ptr as u32)?;
                let _ = caller.data_mut().handles.remove(map_data.data);
                Ok(())
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_data_get_entry",
            |mut caller: Caller<'_, HostState>, map_data_ptr: i32, index: i32, out_ptr: i32| {
                let map_data = read_guest_struct::<wasm::KMapData>(&caller, map_data_ptr as u32)?;
                let Some(WasmHandle::MapData(entries)) =
                    caller.data().handles.get(map_data.data).cloned()
                else {
                    return Err(wasmi::Error::new("invalid wasm map data handle"));
                };
                let Some((key, value)) = entries.get(index as usize).cloned() else {
                    return Err(wasmi::Error::new("invalid wasm map entry index"));
                };
                let key_handle = insert_wasm_handle(caller.data_mut(), WasmHandle::ValueView(key));
                let value_handle =
                    insert_wasm_handle(caller.data_mut(), WasmHandle::ValueView(value));
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KMapEntryView {
                        key: wasm::KValueView(key_handle),
                        value: wasm::KValueView(value_handle),
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_insert_value",
            |mut caller: Caller<'_, HostState>,
             map_ptr: i32,
             key_ptr: i32,
             value_ptr: i32,
             status_ptr: i32| {
                write_status_result(&mut caller, status_ptr as u32, |caller| {
                    let map = read_guest_struct::<wasm::KMap>(&*caller, map_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let key = read_guest_string(caller, key_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let value = read_guest_struct::<wasm::KValue>(&*caller, value_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let value = decode_wasm_value(caller.data_mut(), value)?;
                    let Some(WasmHandle::Map(map_value)) =
                        caller.data().handles.get(map.data).cloned()
                    else {
                        return Err(Error::from("invalid wasm map handle"));
                    };
                    map_value.insert(key.as_str(), value);
                    Ok(())
                })
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_map_insert_meta_value",
            |mut caller: Caller<'_, HostState>,
             map_ptr: i32,
             key_ptr: i32,
             value_ptr: i32,
             status_ptr: i32| {
                write_status_result(&mut caller, status_ptr as u32, |caller| {
                    let map = read_guest_struct::<wasm::KMap>(&*caller, map_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let key = read_guest_struct::<wasm::MetaKey>(&*caller, key_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let value = read_guest_struct::<wasm::KValue>(&*caller, value_ptr as u32)
                        .map_err(wasmi_error_to_runtime)?;
                    let value = decode_wasm_value(caller.data_mut(), value)?;
                    let Some(WasmHandle::Map(mut map_value)) =
                        caller.data().handles.get(map.data).cloned()
                    else {
                        return Err(Error::from("invalid wasm map handle"));
                    };
                    if let Some(meta_key) = decode_meta_key(caller, key)? {
                        map_value.insert_meta(meta_key, value);
                        Ok(())
                    } else {
                        Err(Error::from("unsupported wasm meta key"))
                    }
                })
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(())
}

fn define_callable_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker
        .func_wrap(
            "koto",
            "koto_native_function_make",
            |mut caller: Caller<'_, HostState>,
             symbol_name_ptr: i32,
             user_data: i32,
             out_ptr: i32,
             status_ptr: i32| {
                write_status_value_result(
                    &mut caller,
                    out_ptr as u32,
                    status_ptr as u32,
                    |caller| {
                        let symbol = read_guest_string(caller, symbol_name_ptr as u32)
                            .map_err(wasmi_error_to_runtime)?;
                        let handle = insert_wasm_handle(
                            caller.data_mut(),
                            WasmHandle::NativeFunction(RegisteredFunction {
                                symbol,
                                user_data: user_data as u32,
                            }),
                        );
                        Ok(wasm::KValue {
                            kind: KValueKind::NativeFunction,
                            data: wasm::KValueData {
                                native_function_value: wasm::OpaqueHandle {
                                    data: handle,
                                    metadata: 0,
                                },
                            },
                        })
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(())
}

fn define_object_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker
        .func_wrap(
            "koto",
            "koto_object_make",
            |mut caller: Caller<'_, HostState>, user_data: i32, out_ptr: i32| {
                let handle =
                    insert_wasm_handle(caller.data_mut(), WasmHandle::Object(user_data as u32));
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KObject {
                        data: handle,
                        metadata: user_data as u32,
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    linker
        .func_wrap(
            "koto",
            "koto_iterator_make",
            |mut caller: Caller<'_, HostState>, user_data: i32, out_ptr: i32| {
                let handle =
                    insert_wasm_handle(caller.data_mut(), WasmHandle::Iterator(user_data as u32));
                write_guest_struct(
                    &mut caller,
                    out_ptr as u32,
                    &wasm::KValue {
                        kind: KValueKind::Iterator,
                        data: wasm::KValueData {
                            iterator_value: wasm::OpaqueHandle {
                                data: handle,
                                metadata: user_data as u32,
                            },
                        },
                    },
                )
            },
        )
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(())
}

fn write_status_result<F>(
    caller: &mut Caller<'_, HostState>,
    status_ptr: u32,
    f: F,
) -> Result<(), wasmi::Error>
where
    F: FnOnce(&mut Caller<'_, HostState>) -> Result<(), crate::Error>,
{
    let status = f(caller)
        .map(|()| wasm::KotoStatus::ok())
        .unwrap_or_else(runtime_error_to_status);
    write_guest_struct(caller, status_ptr, &status)
}

fn write_status_value_result<F>(
    caller: &mut Caller<'_, HostState>,
    out_ptr: u32,
    status_ptr: u32,
    f: F,
) -> Result<(), wasmi::Error>
where
    F: FnOnce(&mut Caller<'_, HostState>) -> Result<wasm::KValue, crate::Error>,
{
    let (status, value) = f(caller)
        .map(|value| (wasm::KotoStatus::ok(), value))
        .unwrap_or_else(|error| (runtime_error_to_status(error), wasm::KValue::null()));
    write_guest_struct(caller, out_ptr, &value)?;
    write_guest_struct(caller, status_ptr, &status)
}

fn write_guest_value_slice(
    caller: &mut Caller<'_, HostState>,
    values: Vec<KValue>,
    out_ptr: u32,
) -> Result<(), wasmi::Error> {
    let len = values.len() as u32;
    let data_ptr = write_guest_value_views(caller, values)?;
    let slice = wasm::KValueSlice {
        data: data_ptr,
        len,
        stride: size_of::<wasm::KValueView>() as u32,
    };
    write_guest_struct(caller, out_ptr, &slice)
}

fn write_guest_map_data(
    caller: &mut Caller<'_, HostState>,
    entries: Vec<(KValue, KValue)>,
    out_ptr: u32,
) -> Result<(), wasmi::Error> {
    let len = entries.len() as u32;
    let handle = insert_wasm_handle(caller.data_mut(), WasmHandle::MapData(entries));
    write_guest_struct(caller, out_ptr, &wasm::KMapData { data: handle, len })
}

fn read_guest_tuple(
    caller: &Caller<'_, HostState>,
    tuple_ptr: u32,
) -> Result<crate::KTuple, wasmi::Error> {
    let tuple = read_guest_struct::<wasm::KTuple>(caller, tuple_ptr)?;
    let handle = unsafe { tuple.data.full };
    match caller.data().handles.get(handle) {
        Some(WasmHandle::Tuple(value)) => Ok(value.clone()),
        _ => Err(wasmi::Error::new("invalid wasm tuple handle")),
    }
}

fn read_guest_list(
    caller: &Caller<'_, HostState>,
    list_ptr: u32,
) -> Result<crate::KList, wasmi::Error> {
    let list = read_guest_struct::<wasm::KList>(caller, list_ptr)?;
    match caller.data().handles.get(list.0) {
        Some(WasmHandle::List(value)) => Ok(value.clone()),
        _ => Err(wasmi::Error::new("invalid wasm list handle")),
    }
}

fn read_guest_map(
    caller: &Caller<'_, HostState>,
    map_ptr: u32,
) -> Result<crate::KMap, wasmi::Error> {
    let map = read_guest_struct::<wasm::KMap>(caller, map_ptr)?;
    match caller.data().handles.get(map.data) {
        Some(WasmHandle::Map(value)) => Ok(value.clone()),
        _ => Err(wasmi::Error::new("invalid wasm map handle")),
    }
}

pub(super) fn read_guest_struct<T: Copy>(
    caller: &Caller<'_, HostState>,
    ptr: u32,
) -> Result<T, wasmi::Error> {
    let memory = get_memory(caller)?;
    let mut out = MaybeUninit::<T>::uninit();
    let bytes = unsafe { slice::from_raw_parts_mut(out.as_mut_ptr().cast::<u8>(), size_of::<T>()) };
    memory
        .read(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(unsafe { out.assume_init() })
}

pub(super) fn write_guest_struct<T: Copy>(
    caller: &mut Caller<'_, HostState>,
    ptr: u32,
    value: &T,
) -> Result<(), wasmi::Error> {
    let memory = get_memory(&*caller)?;
    let bytes = unsafe { slice::from_raw_parts(ptr::from_ref(value).cast::<u8>(), size_of::<T>()) };
    memory
        .write(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)
}

fn write_guest_bytes(
    caller: &mut Caller<'_, HostState>,
    ptr: u32,
    bytes: &[u8],
) -> Result<(), wasmi::Error> {
    let memory = get_memory(&*caller)?;
    memory
        .write(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)
}

fn read_guest_values(
    caller: &Caller<'_, HostState>,
    ptr: u32,
    len: usize,
) -> Result<Vec<wasm::KValue>, wasmi::Error> {
    let memory = get_memory(caller)?;
    let mut out = vec![wasm::KValue::null(); len];
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            out.as_mut_ptr().cast::<u8>(),
            size_of::<wasm::KValue>() * len,
        )
    };
    memory
        .read(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(out)
}

fn read_guest_value_views(
    caller: &Caller<'_, HostState>,
    ptr: u32,
    len: usize,
) -> Result<Vec<wasm::KValueView>, wasmi::Error> {
    let memory = get_memory(caller)?;
    let mut out = vec![wasm::KValueView::default(); len];
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            out.as_mut_ptr().cast::<u8>(),
            size_of::<wasm::KValueView>() * len,
        )
    };
    memory
        .read(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(out)
}

fn read_guest_entries(
    caller: &Caller<'_, HostState>,
    ptr: u32,
    len: usize,
) -> Result<Vec<wasm::KotoMapEntry>, wasmi::Error> {
    let memory = get_memory(caller)?;
    let mut out = vec![wasm::KotoMapEntry::default(); len];
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            out.as_mut_ptr().cast::<u8>(),
            size_of::<wasm::KotoMapEntry>() * len,
        )
    };
    memory
        .read(caller, ptr as usize, bytes)
        .map_err(wasmi_error_to_runtime_host)?;
    Ok(out)
}

fn read_guest_string(
    caller: &mut Caller<'_, HostState>,
    string_ptr: u32,
) -> Result<String, wasmi::Error> {
    let slice = read_guest_struct::<wasm::KStringSlice>(&*caller, string_ptr)?;
    let memory = get_memory(&*caller)?;
    let mut bytes = vec![0; slice.len as usize];
    memory
        .read(caller, slice.ptr as usize, &mut bytes)
        .map_err(wasmi_error_to_runtime_host)?;
    String::from_utf8(bytes).map_err(|error| wasmi::Error::new(error.to_string()))
}

fn write_guest_value_views(
    caller: &mut Caller<'_, HostState>,
    values: Vec<KValue>,
) -> Result<u32, wasmi::Error> {
    if values.is_empty() {
        return Ok(0);
    }

    let views = values
        .into_iter()
        .map(|value| {
            wasm::KValueView(
                caller
                    .data_mut()
                    .handles
                    .insert(WasmHandle::ValueView(value)),
            )
        })
        .collect::<Vec<_>>();
    let ptr = guest_alloc(
        caller,
        (size_of::<wasm::KValueView>() * views.len()) as u32,
        align_of::<wasm::KValueView>() as u32,
    )?;
    let bytes = unsafe {
        slice::from_raw_parts(
            views.as_ptr().cast::<u8>(),
            size_of::<wasm::KValueView>() * views.len(),
        )
    };
    write_guest_bytes(caller, ptr, bytes)?;
    Ok(ptr)
}

fn guest_alloc(
    caller: &mut Caller<'_, HostState>,
    size: u32,
    align: u32,
) -> Result<u32, wasmi::Error> {
    let alloc = caller
        .get_export("koto_alloc")
        .and_then(Extern::into_func)
        .ok_or_else(|| wasmi::Error::new("wasm plugin is missing koto_alloc"))?;
    let alloc = alloc
        .typed::<(i32, i32), i32>(&*caller)
        .map_err(wasmi_error_to_runtime_host)?;
    alloc
        .call(caller, (size as i32, align as i32))
        .map(|ptr| ptr as u32)
        .map_err(wasmi_error_to_runtime_host)
}

fn guest_free(
    caller: &mut Caller<'_, HostState>,
    ptr: u32,
    size: usize,
    align: usize,
) -> Result<(), wasmi::Error> {
    let free = caller
        .get_export("koto_free")
        .and_then(Extern::into_func)
        .ok_or_else(|| wasmi::Error::new("wasm plugin is missing koto_free"))?;
    let free = free
        .typed::<(i32, i32, i32), ()>(&*caller)
        .map_err(wasmi_error_to_runtime_host)?;
    free.call(caller, (ptr as i32, size as i32, align as i32))
        .map_err(wasmi_error_to_runtime_host)
}

pub(super) fn get_memory(caller: &Caller<'_, HostState>) -> Result<Memory, wasmi::Error> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmi::Error::new("wasm plugin is missing an exported memory"))
}

fn wasmi_error_to_runtime_host(error: impl std::fmt::Display) -> wasmi::Error {
    wasmi::Error::new(error.to_string())
}
