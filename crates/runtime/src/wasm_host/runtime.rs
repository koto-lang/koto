use crate::{Error, KIterator, KIteratorOutput, KMap, KValue, Result, error::ErrorKind};
use koto_ffi::{IterableKind, KotoStatusCode, wasm};
use parking_lot::Mutex;
use std::{
    fs,
    mem::{MaybeUninit, align_of, size_of},
    path::{Path, PathBuf},
    ptr,
    sync::Arc,
};
use wasmi::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

use super::{
    codec::{
        decode_wasm_value, drop_wasm_value, encode_runtime_value, status_to_runtime_error,
        wasmi_error_to_runtime,
    },
    handles::{
        GuestResource, HostState, call_guest_drop_with_runtime, release_guest_resource_count,
    },
    imports::define_host_imports,
};

pub(super) struct WasmRuntime {
    pub store: Store<HostState>,
    pub instance: Instance,
    pub memory: Memory,
    alloc: TypedFunc<(i32, i32), i32>,
    free: TypedFunc<(i32, i32, i32), ()>,
}

impl WasmRuntime {
    pub fn retain_guest_resource(&mut self, resource: GuestResource) {
        *self
            .store
            .data_mut()
            .guest_resources
            .entry(resource)
            .or_default() += 1;
    }

    pub fn release_guest_resource(&mut self, resource: GuestResource) {
        let should_drop = release_guest_resource_count(self.store.data_mut(), &resource);

        if should_drop {
            let _ = call_guest_drop_with_runtime(self, &resource);
        }
    }

    fn decode_owned_value(&mut self, value: wasm::KValue) -> Result<KValue> {
        let decoded = decode_wasm_value(self.store.data_mut(), value);
        drop_wasm_value(self, value);
        decoded
    }

    fn alloc(&mut self, size: usize, align: usize) -> Result<u32> {
        let ptr = self
            .alloc
            .call(&mut self.store, (size as i32, align as i32))
            .map_err(wasmi_error_to_runtime)?;
        Ok(ptr as u32)
    }

    fn free(&mut self, ptr: u32, size: usize, align: usize) -> Result<()> {
        self.free
            .call(&mut self.store, (ptr as i32, size as i32, align as i32))
            .map_err(wasmi_error_to_runtime)?;
        Ok(())
    }

    fn read_struct<T: Copy>(&self, ptr: u32) -> Result<T> {
        let mut out = MaybeUninit::<T>::uninit();
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(out.as_mut_ptr().cast::<u8>(), size_of::<T>())
        };
        self.memory
            .read(&self.store, ptr as usize, bytes)
            .map_err(|error| Error::from(error.to_string()))?;
        Ok(unsafe { out.assume_init() })
    }

    fn write_struct<T: Copy>(&mut self, ptr: u32, value: &T) -> Result<()> {
        let bytes = unsafe {
            std::slice::from_raw_parts(ptr::from_ref(value).cast::<u8>(), size_of::<T>())
        };
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|error| Error::from(error.to_string()))
    }

    fn write_slice<T: Copy>(&mut self, values: &[T]) -> Result<u32> {
        if values.is_empty() {
            return Ok(0);
        }

        let ptr = self.alloc(size_of_val(values), align_of::<T>())?;
        let bytes = unsafe {
            std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
        };
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|error| Error::from(error.to_string()))?;
        Ok(ptr)
    }

    fn alloc_status_out<T: Copy>(&mut self) -> Result<(u32, u32)> {
        Ok((
            self.alloc(size_of::<T>(), align_of::<T>())?,
            self.alloc(
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?,
        ))
    }

    fn finish_status_only(&mut self, status_ptr: u32) -> Result<()> {
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        if status.code == KotoStatusCode::Ok {
            Ok(())
        } else {
            Err(status_to_runtime_error(&self.memory, &self.store, status))
        }
    }

    fn finish_value_out(&mut self, out_ptr: u32, status_ptr: u32) -> Result<KValue> {
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        let out = self.read_struct::<wasm::KValue>(out_ptr)?;
        self.free(
            out_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        if status.code == KotoStatusCode::Ok {
            self.decode_owned_value(out)
        } else {
            Err(status_to_runtime_error(&self.memory, &self.store, status))
        }
    }

    fn finish_bool_out(&mut self, out_ptr: u32, status_ptr: u32) -> Result<bool> {
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        if status.code != KotoStatusCode::Ok {
            let error = status_to_runtime_error(&self.memory, &self.store, status);
            self.free(out_ptr, size_of::<bool>(), align_of::<bool>())?;
            self.free(
                status_ptr,
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?;
            return Err(error);
        }
        let out = self.read_struct::<bool>(out_ptr)?;
        self.free(out_ptr, size_of::<bool>(), align_of::<bool>())?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        Ok(out)
    }

    fn finish_optional_value_out(
        &mut self,
        out_ptr: u32,
        has_value_ptr: u32,
        status_ptr: u32,
    ) -> Result<Option<KIteratorOutput>> {
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        let has_value = self.read_struct::<bool>(has_value_ptr)?;
        self.free(has_value_ptr, size_of::<bool>(), align_of::<bool>())?;
        if status.code != KotoStatusCode::Ok {
            let error = status_to_runtime_error(&self.memory, &self.store, status);
            self.free(
                out_ptr,
                size_of::<wasm::KValue>(),
                align_of::<wasm::KValue>(),
            )?;
            self.free(
                status_ptr,
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?;
            return Err(error);
        }
        let result = if has_value {
            let out = self.read_struct::<wasm::KValue>(out_ptr)?;
            Some(KIteratorOutput::Value(self.decode_owned_value(out)?))
        } else {
            None
        };
        self.free(
            out_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        Ok(result)
    }

    fn finish_optional_usize_out(
        &mut self,
        out_ptr: u32,
        has_size_ptr: u32,
        status_ptr: u32,
    ) -> Result<Option<usize>> {
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        let has_size = self.read_struct::<bool>(has_size_ptr)?;
        if status.code != KotoStatusCode::Ok {
            let error = status_to_runtime_error(&self.memory, &self.store, status);
            self.free(out_ptr, size_of::<u32>(), align_of::<u32>())?;
            self.free(has_size_ptr, size_of::<bool>(), align_of::<bool>())?;
            self.free(
                status_ptr,
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?;
            return Err(error);
        }
        let result = if has_size {
            Some(self.read_struct::<u32>(out_ptr)? as usize)
        } else {
            None
        };
        self.free(out_ptr, size_of::<u32>(), align_of::<u32>())?;
        self.free(has_size_ptr, size_of::<bool>(), align_of::<bool>())?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        Ok(result)
    }

    fn finish_iterator_out(
        &mut self,
        out_ptr: u32,
        status_ptr: u32,
        context: &str,
    ) -> Result<KIterator> {
        match self.finish_value_out(out_ptr, status_ptr)? {
            KValue::Iterator(iterator) => Ok(iterator),
            unexpected => Err(Error::from(format!(
                "expected Iterator from {context}, found {}",
                unexpected.type_as_string(),
            ))),
        }
    }

    fn write_string_arg(&mut self, value: &str) -> Result<(u32, u32)> {
        let bytes_ptr = self.alloc(value.len(), 1)?;
        self.memory
            .write(&mut self.store, bytes_ptr as usize, value.as_bytes())
            .map_err(|error| Error::from(error.to_string()))?;

        let slice = wasm::KStringSlice {
            ptr: bytes_ptr,
            len: value.len() as u32,
        };
        let slice_ptr = self.alloc(
            size_of::<wasm::KStringSlice>(),
            align_of::<wasm::KStringSlice>(),
        )?;
        self.write_struct(slice_ptr, &slice)?;
        Ok((slice_ptr, bytes_ptr))
    }

    pub(super) fn call_function(
        &mut self,
        symbol: &str,
        user_data: u32,
        instance: KValue,
        args: &[KValue],
    ) -> Result<KValue> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&self.store, symbol)
            .map_err(wasmi_error_to_runtime)?;

        let encoded_args = {
            let state = self.store.data_mut();
            let mut out = Vec::with_capacity(args.len());
            for (index, value) in args.iter().cloned().enumerate() {
                out.push(encode_runtime_value(state, value).map_err(|error| {
                    Error::from(format!("failed to encode wasm argument {index}: {error}"))
                })?);
            }
            out
        };
        let args_ptr = self.write_slice(&encoded_args)?;
        let encoded_instance = {
            let state = self.store.data_mut();
            if matches!(instance, KValue::NativeFunction(_)) {
                wasm::KValue::null()
            } else {
                encode_runtime_value(state, instance).map_err(|error| {
                    Error::from(format!("failed to encode wasm instance: {error}"))
                })?
            }
        };
        let instance_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(instance_ptr, &encoded_instance)?;

        let call_ctx = wasm::CallContext {
            instance: encoded_instance,
            args_ptr,
            arg_count: encoded_args.len() as u32,
        };
        let call_ctx_ptr = self.alloc(
            size_of::<wasm::CallContext>(),
            align_of::<wasm::CallContext>(),
        )?;
        self.write_struct(call_ctx_ptr, &call_ctx)?;

        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;

        func.call(
            &mut self.store,
            (
                call_ctx_ptr as i32,
                user_data as i32,
                out_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;

        for value in &encoded_args {
            drop_wasm_value(self, *value);
        }
        drop_wasm_value(self, encoded_instance);
        self.free(
            args_ptr,
            size_of::<wasm::KValue>() * encoded_args.len(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(
            instance_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(
            call_ctx_ptr,
            size_of::<wasm::CallContext>(),
            align_of::<wasm::CallContext>(),
        )?;

        self.finish_value_out(out_ptr, status_ptr)
    }

    pub(super) fn object_type_string(&mut self, user_data: u32) -> Result<crate::KString> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(&self.store, "koto_plugin_object_type_string_v1")
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KStringSlice>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        if status.code != KotoStatusCode::Ok {
            let error = status_to_runtime_error(&self.memory, &self.store, status);
            self.free(
                out_ptr,
                size_of::<wasm::KStringSlice>(),
                align_of::<wasm::KStringSlice>(),
            )?;
            self.free(
                status_ptr,
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?;
            return Err(error);
        }
        let slice = self.read_struct::<wasm::KStringSlice>(out_ptr)?;
        let bytes = self.memory.data(&self.store)
            [slice.ptr as usize..(slice.ptr + slice.len) as usize]
            .to_vec();
        self.free(
            out_ptr,
            size_of::<wasm::KStringSlice>(),
            align_of::<wasm::KStringSlice>(),
        )?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        Ok(String::from_utf8(bytes)
            .map_err(|error| Error::from(error.to_string()))?
            .into())
    }

    pub(super) fn object_named_value(
        &mut self,
        user_data: u32,
        key: &str,
    ) -> Result<Option<KValue>> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_named_value_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let (key_ptr, key_bytes_ptr) = self.write_string_arg(key)?;
        let out_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        let found_ptr = self.alloc(size_of::<bool>(), align_of::<bool>())?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                key_ptr as i32,
                out_ptr as i32,
                found_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        let status = self.read_struct::<wasm::KotoStatus>(status_ptr)?;
        if status.code != KotoStatusCode::Ok {
            let error = status_to_runtime_error(&self.memory, &self.store, status);
            self.free(
                key_ptr,
                size_of::<wasm::KStringSlice>(),
                align_of::<wasm::KStringSlice>(),
            )?;
            self.free(key_bytes_ptr, key.len(), 1)?;
            self.free(
                out_ptr,
                size_of::<wasm::KValue>(),
                align_of::<wasm::KValue>(),
            )?;
            self.free(found_ptr, size_of::<bool>(), align_of::<bool>())?;
            self.free(
                status_ptr,
                size_of::<wasm::KotoStatus>(),
                align_of::<wasm::KotoStatus>(),
            )?;
            return Err(error);
        }
        let found = self.read_struct::<bool>(found_ptr)?;
        let result = if found {
            let out = self.read_struct::<wasm::KValue>(out_ptr)?;
            Some(self.decode_owned_value(out)?)
        } else {
            None
        };
        self.free(
            key_ptr,
            size_of::<wasm::KStringSlice>(),
            align_of::<wasm::KStringSlice>(),
        )?;
        self.free(key_bytes_ptr, key.len(), 1)?;
        self.free(
            out_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(found_ptr, size_of::<bool>(), align_of::<bool>())?;
        self.free(
            status_ptr,
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        Ok(result)
    }

    pub(super) fn object_named_value_assign(
        &mut self,
        user_data: u32,
        key: &str,
        value: &KValue,
    ) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_named_value_assign_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let (key_ptr, key_bytes_ptr) = self.write_string_arg(key)?;
        let encoded = {
            let state = self.store.data_mut();
            encode_runtime_value(state, value.clone())?
        };
        let value_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(value_ptr, &encoded)?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                key_ptr as i32,
                value_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        drop_wasm_value(self, encoded);
        self.free(
            key_ptr,
            size_of::<wasm::KStringSlice>(),
            align_of::<wasm::KStringSlice>(),
        )?;
        self.free(key_bytes_ptr, key.len(), 1)?;
        self.free(
            value_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.finish_status_only(status_ptr)
    }

    pub(super) fn object_display(&mut self, user_data: u32) -> Result<KValue> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(&self.store, "koto_plugin_object_display_v1")
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_value_out(out_ptr, status_ptr)
    }

    pub(super) fn object_call(
        &mut self,
        user_data: u32,
        ctx: &mut crate::CallContext,
    ) -> Result<KValue> {
        self.call_function(
            "koto_plugin_object_call_v1",
            user_data,
            ctx.instance().clone(),
            ctx.args(),
        )
    }

    pub(super) fn object_size(&mut self, user_data: u32) -> Result<Option<usize>> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&self.store, "koto_plugin_object_size_v1")
            .map_err(wasmi_error_to_runtime)?;
        let out_ptr = self.alloc(size_of::<u32>(), align_of::<u32>())?;
        let has_size_ptr = self.alloc(size_of::<bool>(), align_of::<bool>())?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                out_ptr as i32,
                has_size_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_optional_usize_out(out_ptr, has_size_ptr, status_ptr)
    }

    pub(super) fn object_is_callable(&mut self, user_data: u32) -> Result<bool> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(&self.store, "koto_plugin_object_is_callable_v1")
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<bool>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_bool_out(out_ptr, status_ptr)
    }

    pub(super) fn object_unary_op(
        &mut self,
        user_data: u32,
        op: koto_api::UnaryOp,
    ) -> Result<KValue> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_unary_op_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                op as i32,
                out_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_value_out(out_ptr, status_ptr)
    }

    pub(super) fn object_binary_op(
        &mut self,
        user_data: u32,
        op: koto_api::BinaryOp,
        rhs: &KValue,
    ) -> Result<KValue> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_binary_op_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let encoded_rhs = {
            let state = self.store.data_mut();
            encode_runtime_value(state, rhs.clone())?
        };
        let rhs_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(rhs_ptr, &encoded_rhs)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                op as i32,
                rhs_ptr as i32,
                out_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        drop_wasm_value(self, encoded_rhs);
        self.free(
            rhs_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.finish_value_out(out_ptr, status_ptr)
    }

    pub(super) fn object_index(&mut self, user_data: u32, index: &KValue) -> Result<KValue> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&self.store, "koto_plugin_object_index_v1")
            .map_err(wasmi_error_to_runtime)?;
        let encoded_index = {
            let state = self.store.data_mut();
            encode_runtime_value(state, index.clone())?
        };
        let index_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(index_ptr, &encoded_index)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                index_ptr as i32,
                out_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        drop_wasm_value(self, encoded_index);
        self.free(
            index_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.finish_value_out(out_ptr, status_ptr)
    }

    pub(super) fn object_index_assign(
        &mut self,
        user_data: u32,
        index: &KValue,
        value: &KValue,
    ) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_index_assign_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let encoded_index = {
            let state = self.store.data_mut();
            encode_runtime_value(state, index.clone())?
        };
        let encoded_value = {
            let state = self.store.data_mut();
            encode_runtime_value(state, value.clone())?
        };
        let index_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        let value_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(index_ptr, &encoded_index)?;
        self.write_struct(value_ptr, &encoded_value)?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                index_ptr as i32,
                value_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        drop_wasm_value(self, encoded_index);
        drop_wasm_value(self, encoded_value);
        self.free(
            index_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.free(
            value_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.finish_status_only(status_ptr)
    }

    pub(super) fn object_binary_op_assign(
        &mut self,
        user_data: u32,
        op: koto_api::BinaryOp,
        rhs: &KValue,
    ) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_binary_op_assign_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let encoded_rhs = {
            let state = self.store.data_mut();
            encode_runtime_value(state, rhs.clone())?
        };
        let rhs_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        self.write_struct(rhs_ptr, &encoded_rhs)?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                op as i32,
                rhs_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        drop_wasm_value(self, encoded_rhs);
        self.free(
            rhs_ptr,
            size_of::<wasm::KValue>(),
            align_of::<wasm::KValue>(),
        )?;
        self.finish_status_only(status_ptr)
    }

    pub(super) fn object_iterable_kind(&mut self, user_data: u32) -> Result<crate::IsIterable> {
        let func = self
            .instance
            .get_typed_func::<i32, i32>(&self.store, "koto_plugin_object_iterable_kind_v1")
            .map_err(wasmi_error_to_runtime)?;
        match func
            .call(&mut self.store, user_data as i32)
            .map_err(wasmi_error_to_runtime)?
        {
            x if x == IterableKind::NotIterable as i32 => Ok(crate::IsIterable::NotIterable),
            x if x == IterableKind::Iterable as i32 => Ok(crate::IsIterable::Iterable),
            x if x == IterableKind::ForwardIterator as i32 => {
                Ok(crate::IsIterable::ForwardIterator)
            }
            x if x == IterableKind::BidirectionalIterator as i32 => {
                Ok(crate::IsIterable::BidirectionalIterator)
            }
            _ => Err(Error::from(
                "invalid iterable kind returned by wasm plugin object",
            )),
        }
    }

    pub(super) fn object_make_iterator(&mut self, user_data: u32) -> Result<KIterator> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_object_make_iterator_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_iterator_out(out_ptr, status_ptr, "wasm object @iterator")
    }

    pub(super) fn iterator_next(&mut self, user_data: u32) -> Result<Option<KIteratorOutput>> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&self.store, "koto_plugin_iterator_next_v1")
            .map_err(wasmi_error_to_runtime)?;
        let out_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        let has_value_ptr = self.alloc(size_of::<bool>(), align_of::<bool>())?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                out_ptr as i32,
                has_value_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_optional_value_out(out_ptr, has_value_ptr, status_ptr)
    }

    pub(super) fn iterator_is_bidirectional(&mut self, user_data: u32) -> Result<bool> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_iterator_is_bidirectional_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<bool>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_bool_out(out_ptr, status_ptr)
    }

    pub(super) fn iterator_copy(&mut self, user_data: u32) -> Result<KIterator> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(&self.store, "koto_plugin_iterator_copy_v1")
            .map_err(wasmi_error_to_runtime)?;
        let (out_ptr, status_ptr) = self.alloc_status_out::<wasm::KValue>()?;
        func.call(
            &mut self.store,
            (user_data as i32, out_ptr as i32, status_ptr as i32),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_iterator_out(out_ptr, status_ptr, "wasm iterator copy")
    }

    pub(super) fn iterator_next_back(&mut self, user_data: u32) -> Result<Option<KIteratorOutput>> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &self.store,
                "koto_plugin_iterator_next_back_v1",
            )
            .map_err(wasmi_error_to_runtime)?;
        let out_ptr = self.alloc(size_of::<wasm::KValue>(), align_of::<wasm::KValue>())?;
        let has_value_ptr = self.alloc(size_of::<bool>(), align_of::<bool>())?;
        let status_ptr = self.alloc(
            size_of::<wasm::KotoStatus>(),
            align_of::<wasm::KotoStatus>(),
        )?;
        func.call(
            &mut self.store,
            (
                user_data as i32,
                out_ptr as i32,
                has_value_ptr as i32,
                status_ptr as i32,
            ),
        )
        .map_err(wasmi_error_to_runtime)?;
        self.finish_optional_value_out(out_ptr, has_value_ptr, status_ptr)
    }
}

pub(crate) fn is_wasm_import(import_name: &str) -> bool {
    import_name.starts_with("wasm:")
}

pub(crate) fn resolve_wasm_module_path(
    import_name: &str,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let Some(raw_path) = import_name.strip_prefix("wasm:") else {
        return Err(Error::from("wasm import is missing the 'wasm:' prefix"));
    };

    let module_path = Path::new(raw_path);

    if module_path.is_absolute() {
        Ok(module_path.to_path_buf())
    } else if let Some(source_path) = source_path {
        match source_path.parent() {
            Some(parent) => Ok(parent.join(module_path)),
            None => Ok(module_path.to_path_buf()),
        }
    } else {
        Ok(module_path.to_path_buf())
    }
}

pub(crate) fn load_wasm_module(module_path: &Path) -> Result<KMap> {
    let wasm_bytes = fs::read(module_path).map_err(|error| {
        Error::from(format!(
            "failed to read wasm module '{}': {error}",
            module_path.display()
        ))
    })?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes[..]).map_err(wasmi_error_to_runtime)?;
    let host_state = HostState::default();
    let runtime_target = host_state.runtime_target.clone();
    let mut store = Store::new(&engine, host_state);
    let mut linker = Linker::new(&engine);
    define_host_imports(&mut linker).map_err(wasmi_error_to_runtime)?;

    let instance = linker
        .instantiate(&mut store, &module)
        .and_then(|pre| pre.start(&mut store))
        .map_err(wasmi_error_to_runtime)?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or_else(|| Error::from("wasm plugin is missing an exported memory"))?;
    let alloc = instance
        .get_typed_func::<(i32, i32), i32>(&store, "koto_alloc")
        .map_err(wasmi_error_to_runtime)?;
    let free = instance
        .get_typed_func::<(i32, i32, i32), ()>(&store, "koto_free")
        .map_err(wasmi_error_to_runtime)?;

    let runtime = Arc::new(Mutex::new(WasmRuntime {
        store,
        instance,
        memory,
        alloc,
        free,
    }));
    *runtime_target.lock() = Some(Arc::downgrade(&runtime));

    let mut runtime_lock = runtime.lock();
    let init = runtime_lock
        .instance
        .get_typed_func::<(i32, i32), ()>(&runtime_lock.store, "koto_plugin_init_v1")
        .map_err(wasmi_error_to_runtime)?;

    let (out_ptr, status_ptr) = runtime_lock.alloc_status_out::<wasm::KValue>()?;
    init.call(&mut runtime_lock.store, (out_ptr as i32, status_ptr as i32))
        .map_err(wasmi_error_to_runtime)?;

    match runtime_lock.finish_value_out(out_ptr, status_ptr)? {
        KValue::Map(map) => Ok(map),
        other => Err(Error::new(ErrorKind::UnexpectedType {
            expected: "Map".into(),
            unexpected: other,
        })),
    }
}
