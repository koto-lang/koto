use crate::{
    DisplayContext, Error, KIterator, KIteratorOutput, KNativeFunction, KObject, KValue, Result,
    RuntimeBackend, types::KotoIterator,
};
use koto_api::{BinaryOp, KotoAccess, KotoCopy, KotoObjectOps, KotoStaticType, KotoType, UnaryOp};
use parking_lot::Mutex;
use std::sync::Arc;

use super::{
    handles::{GuestResource, lookup_runtime_wasm_function, register_runtime_wasm_function},
    runtime::WasmRuntime,
};

pub(super) struct WasmObjectData {
    pub handle: u32,
    pub user_data: u32,
    pub runtime: Arc<Mutex<WasmRuntime>>,
}

pub(super) struct WasmIteratorData {
    pub user_data: u32,
    pub runtime: Arc<Mutex<WasmRuntime>>,
}

pub(super) struct WasmNativeFunctionGuard {
    pub user_data: u32,
    pub runtime: Arc<Mutex<WasmRuntime>>,
}

impl WasmObjectData {
    fn make_object(&self) -> KObject {
        self.clone().into()
    }

    fn type_name(&self) -> Result<crate::KString> {
        self.runtime.lock().object_type_string(self.user_data)
    }
}

impl Clone for WasmObjectData {
    fn clone(&self) -> Self {
        self.runtime
            .lock()
            .retain_guest_resource(GuestResource::Object(self.user_data));

        Self {
            handle: self.handle,
            user_data: self.user_data,
            runtime: self.runtime.clone(),
        }
    }
}

impl Drop for WasmObjectData {
    fn drop(&mut self) {
        self.runtime
            .lock()
            .release_guest_resource(GuestResource::Object(self.user_data));
    }
}

impl Clone for WasmIteratorData {
    fn clone(&self) -> Self {
        self.runtime
            .lock()
            .retain_guest_resource(GuestResource::Iterator(self.user_data));

        Self {
            user_data: self.user_data,
            runtime: self.runtime.clone(),
        }
    }
}

impl Drop for WasmIteratorData {
    fn drop(&mut self) {
        self.runtime
            .lock()
            .release_guest_resource(GuestResource::Iterator(self.user_data));
    }
}

impl Drop for WasmNativeFunctionGuard {
    fn drop(&mut self) {
        self.runtime
            .lock()
            .release_guest_resource(GuestResource::NativeFunction(self.user_data));
    }
}

pub(super) fn make_wasm_object(
    handle: u32,
    user_data: u32,
    runtime: Arc<Mutex<WasmRuntime>>,
) -> KObject {
    WasmObjectData {
        handle,
        user_data,
        runtime,
    }
    .into()
}

impl KotoStaticType for WasmObjectData {
    fn type_static() -> &'static str {
        "WasmObject"
    }
}

impl KotoType<RuntimeBackend> for WasmObjectData {
    fn type_string(&self) -> crate::KString {
        self.type_name().unwrap_or_else(|_| "Object".into())
    }
}

impl KotoCopy<RuntimeBackend> for WasmObjectData {
    fn copy(&self) -> KObject {
        self.make_object()
    }
}

impl KotoAccess<RuntimeBackend> for WasmObjectData {
    fn access(&self, key: &crate::KString) -> Result<Option<KValue>> {
        self.runtime
            .lock()
            .object_named_value(self.user_data, key.as_str())
    }

    fn access_assign(&mut self, key: &crate::KString, value: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_named_value_assign(self.user_data, key.as_str(), value)
    }
}

impl KotoObjectOps<RuntimeBackend> for WasmObjectData {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        let value = self.runtime.lock().object_display(self.user_data)?;
        value.display(ctx)
    }

    fn negate(&self) -> Result<KValue> {
        self.runtime
            .lock()
            .object_unary_op(self.user_data, UnaryOp::Negate)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        self.runtime.lock().object_index(self.user_data, index)
    }

    fn index_assign(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_index_assign(self.user_data, index, value)
    }

    fn size(&self) -> Result<Option<usize>> {
        self.runtime.lock().object_size(self.user_data)
    }

    fn is_callable(&self) -> Result<bool> {
        self.runtime.lock().object_is_callable(self.user_data)
    }

    fn call(&mut self, ctx: &mut crate::CallContext) -> Result<KValue> {
        self.runtime.lock().object_call(self.user_data, ctx)
    }

    fn add(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Add, other)
    }

    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::AddRhs, other)
    }

    fn subtract(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Subtract, other)
    }

    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::SubtractRhs, other)
    }

    fn multiply(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Multiply, other)
    }

    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::MultiplyRhs, other)
    }

    fn divide(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Divide, other)
    }

    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::DivideRhs, other)
    }

    fn remainder(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Remainder, other)
    }

    fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::RemainderRhs, other)
    }

    fn power(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Power, other)
    }

    fn power_rhs(&self, other: &KValue) -> Result<KValue> {
        self.runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::PowerRhs, other)
    }

    fn equal(&self, other: &KValue) -> Result<bool> {
        match self
            .runtime
            .lock()
            .object_binary_op(self.user_data, BinaryOp::Equal, other)?
        {
            KValue::Bool(result) => Ok(result),
            unexpected => Err(Error::from(format!(
                "expected Bool from wasm object equality, found {}",
                unexpected.type_as_string(),
            ))),
        }
    }

    fn add_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_binary_op_assign(self.user_data, BinaryOp::AddAssign, other)
    }

    fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_binary_op_assign(self.user_data, BinaryOp::SubtractAssign, other)
    }

    fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_binary_op_assign(self.user_data, BinaryOp::MultiplyAssign, other)
    }

    fn divide_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_binary_op_assign(self.user_data, BinaryOp::DivideAssign, other)
    }

    fn remainder_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime.lock().object_binary_op_assign(
            self.user_data,
            BinaryOp::RemainderAssign,
            other,
        )
    }

    fn power_assign(&mut self, other: &KValue) -> Result<()> {
        self.runtime
            .lock()
            .object_binary_op_assign(self.user_data, BinaryOp::PowerAssign, other)
    }

    fn is_iterable(&self) -> Result<crate::IsIterable> {
        self.runtime.lock().object_iterable_kind(self.user_data)
    }

    fn make_iterator(&self, _vm: &mut crate::KotoVm) -> Result<KIterator> {
        self.runtime.lock().object_make_iterator(self.user_data)
    }
}

impl KotoIterator for WasmIteratorData {
    fn make_copy(&self) -> Result<KIterator> {
        self.runtime.lock().iterator_copy(self.user_data)
    }

    fn is_bidirectional(&self) -> bool {
        self.runtime
            .lock()
            .iterator_is_bidirectional(self.user_data)
            .unwrap_or(false)
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        match self.runtime.lock().iterator_next_back(self.user_data) {
            Ok(Some(output)) => Some(output),
            Ok(None) => None,
            Err(error) => Some(KIteratorOutput::Error(error)),
        }
    }
}

impl Iterator for WasmIteratorData {
    type Item = KIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        match self.runtime.lock().iterator_next(self.user_data) {
            Ok(Some(output)) => Some(output),
            Ok(None) => None,
            Err(error) => Some(KIteratorOutput::Error(error)),
        }
    }
}

pub(super) fn decode_wasm_native_function(
    registered: super::handles::RegisteredFunction,
    runtime: Arc<Mutex<WasmRuntime>>,
) -> KValue {
    let registered_for_function = registered.clone();
    let runtime_for_function = runtime.clone();
    let function_guard = WasmNativeFunctionGuard {
        user_data: registered.user_data,
        runtime: runtime.clone(),
    };
    let function = KNativeFunction::new(move |ctx| {
        let _guard = &function_guard;
        runtime_for_function.lock().call_function(
            &registered_for_function.symbol,
            registered_for_function.user_data,
            ctx.instance().clone(),
            ctx.args(),
        )
    });
    register_runtime_wasm_function(&function, registered);
    KValue::NativeFunction(function)
}

pub(super) fn encode_runtime_native_function(
    function: &KNativeFunction,
) -> Option<super::handles::RegisteredFunction> {
    lookup_runtime_wasm_function(function).map(|runtime| runtime.registered)
}
