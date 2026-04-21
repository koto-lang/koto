use crate::abi;
use crate::{
    KValue, Result,
    api::{BinaryOp, ReadOp, UnaryOp, WriteOp},
};
use koto_api::KotoVmTrait;
cfg_select! {
    target_arch = "wasm32" => {
        use crate::Error;
    }
    _ => {
        use crate::{host::status_to_error, types::encode_value};
    }
}

pub(crate) fn abi_unary_op(value: UnaryOp) -> abi::UnaryOp {
    match value {
        UnaryOp::Debug => abi::UnaryOp::Debug,
        UnaryOp::Display => abi::UnaryOp::Display,
        UnaryOp::Negate => abi::UnaryOp::Negate,
        UnaryOp::Iterator => abi::UnaryOp::Iterator,
        UnaryOp::Next => abi::UnaryOp::Next,
        UnaryOp::NextBack => abi::UnaryOp::NextBack,
        UnaryOp::Size => abi::UnaryOp::Size,
    }
}

pub(crate) fn abi_binary_op(value: BinaryOp) -> abi::BinaryOp {
    match value {
        BinaryOp::Add => abi::BinaryOp::Add,
        BinaryOp::Subtract => abi::BinaryOp::Subtract,
        BinaryOp::Multiply => abi::BinaryOp::Multiply,
        BinaryOp::Divide => abi::BinaryOp::Divide,
        BinaryOp::Remainder => abi::BinaryOp::Remainder,
        BinaryOp::Power => abi::BinaryOp::Power,
        BinaryOp::AddRhs => abi::BinaryOp::AddRhs,
        BinaryOp::SubtractRhs => abi::BinaryOp::SubtractRhs,
        BinaryOp::MultiplyRhs => abi::BinaryOp::MultiplyRhs,
        BinaryOp::DivideRhs => abi::BinaryOp::DivideRhs,
        BinaryOp::RemainderRhs => abi::BinaryOp::RemainderRhs,
        BinaryOp::PowerRhs => abi::BinaryOp::PowerRhs,
        BinaryOp::AddAssign => abi::BinaryOp::AddAssign,
        BinaryOp::SubtractAssign => abi::BinaryOp::SubtractAssign,
        BinaryOp::MultiplyAssign => abi::BinaryOp::MultiplyAssign,
        BinaryOp::DivideAssign => abi::BinaryOp::DivideAssign,
        BinaryOp::RemainderAssign => abi::BinaryOp::RemainderAssign,
        BinaryOp::PowerAssign => abi::BinaryOp::PowerAssign,
        BinaryOp::Less => abi::BinaryOp::Less,
        BinaryOp::LessOrEqual => abi::BinaryOp::LessOrEqual,
        BinaryOp::Greater => abi::BinaryOp::Greater,
        BinaryOp::GreaterOrEqual => abi::BinaryOp::GreaterOrEqual,
        BinaryOp::Equal => abi::BinaryOp::Equal,
        BinaryOp::NotEqual => abi::BinaryOp::NotEqual,
    }
}

pub(crate) fn abi_read_op(value: ReadOp) -> abi::ReadOp {
    match value {
        ReadOp::Index => abi::ReadOp::Index,
        ReadOp::Access => abi::ReadOp::Access,
    }
}

pub(crate) fn abi_write_op(value: WriteOp) -> abi::WriteOp {
    match value {
        WriteOp::IndexAssign => abi::WriteOp::IndexAssign,
        WriteOp::AccessAssign => abi::WriteOp::AccessAssign,
    }
}

/// A VM facade backed by the active host callback.
#[derive(Clone, Copy, Debug)]
pub struct KotoVm {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    api: *const abi::KotoHostApiV1,
}

unsafe impl Send for KotoVm {}

unsafe impl Sync for KotoVm {}

impl KotoVm {
    pub(crate) fn from_api(api: &abi::KotoHostApiV1) -> Self {
        Self {
            api: api as *const _,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_wasm() -> Self {
        Self {
            api: std::ptr::null(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn api(&self) -> &abi::KotoHostApiV1 {
        assert!(
            !self.api.is_null(),
            "plugin VM operations aren't implemented for wasm yet"
        );
        unsafe { &*self.api }
    }
}

#[cfg(target_arch = "wasm32")]
fn unsupported_wasm_vm_op<T>() -> Result<T> {
    Err(Error::from(
        "plugin VM operations aren't implemented for wasm yet",
    ))
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn decode_vm_result(api: &abi::KotoHostApiV1, value: abi::KValue) -> Result<KValue> {
    let result = crate::types::decode_value(api, value);
    unsafe { (api.value_free)(value) };
    result
}

impl KotoVmTrait<crate::PluginBackend> for KotoVm {
    fn spawn_shared_vm(&self) -> Self {
        *self
    }

    fn call_function(&mut self, function: KValue, args: &[KValue]) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (function, args);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let function = encode_value(api, function);
                let args = args
                    .iter()
                    .cloned()
                    .map(|arg| encode_value(api, arg))
                    .collect::<Vec<_>>();
                let mut out = abi::KValue::default();
                let status =
                    unsafe { (api.vm_call_function)(function, args.as_ptr(), args.len(), &mut out) };

                unsafe {
                    (api.value_free)(function);
                    for arg in args {
                        (api.value_free)(arg);
                    }
                }

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }

    fn call_instance_function(
        &mut self,
        instance: KValue,
        function: KValue,
        args: &[KValue],
    ) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (instance, function, args);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let instance = encode_value(api, instance);
                let function = encode_value(api, function);
                let args = args
                    .iter()
                    .cloned()
                    .map(|arg| encode_value(api, arg))
                    .collect::<Vec<_>>();
                let mut out = abi::KValue::default();
                let status = unsafe {
                    (api.vm_call_instance_function)(
                        instance,
                        function,
                        args.as_ptr(),
                        args.len(),
                        &mut out,
                    )
                };

                unsafe {
                    (api.value_free)(instance);
                    (api.value_free)(function);
                    for arg in args {
                        (api.value_free)(arg);
                    }
                }

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }

    fn run_unary_op(&mut self, op: UnaryOp, value: KValue) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (op, value);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let value = encode_value(api, value);
                let mut out = abi::KValue::default();
                let status = unsafe { (api.vm_run_unary_op)(abi_unary_op(op), value, &mut out) };
                unsafe { (api.value_free)(value) };

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }

    fn run_binary_op(&mut self, op: BinaryOp, lhs: KValue, rhs: KValue) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (op, lhs, rhs);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let lhs = encode_value(api, lhs);
                let rhs = encode_value(api, rhs);
                let mut out = abi::KValue::default();
                let status = unsafe { (api.vm_run_binary_op)(abi_binary_op(op), lhs, rhs, &mut out) };

                unsafe {
                    (api.value_free)(lhs);
                    (api.value_free)(rhs);
                }

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }

    fn run_read_op(&mut self, op: ReadOp, container: KValue, read_arg: KValue) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (op, container, read_arg);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let container = encode_value(api, container);
                let read_arg = encode_value(api, read_arg);
                let mut out = abi::KValue::default();
                let status =
                    unsafe { (api.vm_run_read_op)(abi_read_op(op), container, read_arg, &mut out) };

                unsafe {
                    (api.value_free)(container);
                    (api.value_free)(read_arg);
                }

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }

    fn run_write_op(
        &mut self,
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
    ) -> Result<KValue> {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (op, container, write_arg, write_value);
                unsupported_wasm_vm_op()
            }
            _ => {
                let api = self.api();
                let container = encode_value(api, container);
                let write_arg = encode_value(api, write_arg);
                let write_value = encode_value(api, write_value);
                let mut out = abi::KValue::default();
                let status = unsafe {
                    (api.vm_run_write_op)(
                        abi_write_op(op),
                        container,
                        write_arg,
                        write_value,
                        &mut out,
                    )
                };

                unsafe {
                    (api.value_free)(container);
                    (api.value_free)(write_arg);
                    (api.value_free)(write_value);
                }

                if status.code == abi::KotoStatusCode::Ok {
                    decode_vm_result(api, out)
                } else {
                    Err(status_to_error(status))
                }
            }
        }
    }
}
