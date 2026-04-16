use super::*;
use crate::plugin_host::transfer::AbiTransfer;
use crate::{CallContext, KIteratorOutput, RuntimeBackend};
use koto_api::{KotoAccess, KotoCopy, KotoObjectOps, KotoStaticType};

pub(crate) struct PluginObjectData {
    pub(crate) object_v1: *const abi::KotoPluginObjectV1,
    pub(crate) storage: PtrMut<PluginObjectStorage>,
    pub(crate) library: Arc<LoadedLibrary>,
}

unsafe impl Send for PluginObjectData {}
unsafe impl Sync for PluginObjectData {}

impl PluginObjectData {
    fn object_v1(&self) -> &'static abi::KotoPluginObjectV1 {
        unsafe { &*self.object_v1 }
    }

    fn object_handle(&self) -> PluginObjectHandle {
        PluginObjectHandle(unsafe {
            make_plugin_object(self.object_v1, self.storage.clone(), self.library.clone())
                .into_abi()
        })
    }

    fn type_name(&self) -> KString {
        let object = self.object_handle();
        with_current_library(self.library.clone(), || {
            string_slice_to_string(unsafe {
                (self.object_v1().type_string)(&HOST_API, object.as_abi())
            })
            .into()
        })
    }
}

struct PluginObjectHandle(abi::KObject);

impl PluginObjectHandle {
    fn as_abi(&self) -> abi::KObject {
        self.0
    }
}

impl Drop for PluginObjectHandle {
    fn drop(&mut self) {
        let _ = unsafe { KObject::from_abi(self.0) };
    }
}

pub(crate) fn make_plugin_object(
    object_v1: *const abi::KotoPluginObjectV1,
    storage: PtrMut<PluginObjectStorage>,
    library: Arc<LoadedLibrary>,
) -> KObject {
    PluginObjectData {
        object_v1,
        storage,
        library,
    }
    .into()
}

pub(crate) fn plugin_object_v1(object: &KObject) -> Option<*const abi::KotoPluginObjectV1> {
    object.try_borrow().ok().and_then(|object| {
        (&*object as &dyn std::any::Any)
            .downcast_ref::<PluginObjectData>()
            .map(|data| data.object_v1)
    })
}

impl KotoStaticType for PluginObjectData {
    fn type_static() -> &'static str {
        "PluginObject"
    }
}

fn plugin_object_access(data: &PluginObjectData, key: &KString) -> Result<Option<KValue>> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let mut found = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().named_value)(
            &HOST_API,
            object.as_abi(),
            abi_string_slice(key.as_str()),
            &mut out,
            &mut found,
        )
    });

    if status.code != abi::KotoStatusCode::Ok {
        return Err(status_to_error(status));
    }

    if found {
        return Ok(Some(take_value(out)));
    }

    Ok(None)
}

fn plugin_object_access_assign(
    data: &PluginObjectData,
    key: &KString,
    value: &KValue,
) -> Result<()> {
    let object = data.object_handle();
    let value = encode_abi_value(value.clone());
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().named_value_assign)(
            &HOST_API,
            object.as_abi(),
            abi_string_slice(key.as_str()),
            value,
        )
    });

    unsafe { value_free(value) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_display(data: &PluginObjectData, ctx: &mut DisplayContext) -> Result<()> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().display)(&HOST_API, object.as_abi(), &mut out)
    });

    if status.code != abi::KotoStatusCode::Ok {
        return Err(status_to_error(status));
    }

    take_value(out).display(ctx)
}

fn plugin_object_index(data: &PluginObjectData, index: &KValue) -> Result<KValue> {
    let object = data.object_handle();
    let index = encode_abi_value(index.clone());
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().index)(&HOST_API, object.as_abi(), index, &mut out)
    });
    unsafe { value_free(index) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_index_assign(
    data: &PluginObjectData,
    index: &KValue,
    value: &KValue,
) -> Result<()> {
    let object = data.object_handle();
    let index = encode_abi_value(index.clone());
    let value = encode_abi_value(value.clone());
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().index_assign)(&HOST_API, object.as_abi(), index, value)
    });
    unsafe {
        value_free(index);
        value_free(value);
    }

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_size(data: &PluginObjectData) -> Result<Option<usize>> {
    let object = data.object_handle();
    let mut out = 0;
    let mut has_value = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().size)(&HOST_API, object.as_abi(), &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok || !has_value {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

fn plugin_object_is_callable(data: &PluginObjectData) -> Result<bool> {
    let object = data.object_handle();
    let mut out = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().is_callable)(&HOST_API, object.as_abi(), &mut out)
    });

    if status.code == abi::KotoStatusCode::Ok {
        Ok(out)
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_call(data: &PluginObjectData, ctx: &mut CallContext) -> Result<KValue> {
    let object = data.object_handle();
    let instance = encode_abi_value(ctx.instance().clone());
    let args = ctx
        .args()
        .iter()
        .cloned()
        .map(encode_abi_value)
        .collect::<Vec<_>>();
    let abi_ctx = abi::CallContext {
        instance,
        args: args.as_ptr(),
        arg_count: args.len(),
    };
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || {
        with_current_vm(ctx.vm, || unsafe {
            (data.object_v1().call)(&HOST_API, object.as_abi(), abi_ctx, &mut out)
        })
    });

    unsafe { value_free(instance) };
    for arg in args {
        unsafe { value_free(arg) };
    }

    if status.code == abi::KotoStatusCode::Ok {
        Ok(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_negate(data: &PluginObjectData) -> Result<KValue> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().unary_op)(&HOST_API, object.as_abi(), abi::UnaryOp::Negate, &mut out)
    });

    if status.code == abi::KotoStatusCode::Ok {
        Ok(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_binary_op(
    data: &PluginObjectData,
    op: abi::BinaryOp,
    rhs: &KValue,
    _fn_name: &'static str,
) -> Result<KValue> {
    let object = data.object_handle();
    let rhs = encode_abi_value(rhs.clone());
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().binary_op)(&HOST_API, object.as_abi(), op, rhs, &mut out)
    });
    unsafe { value_free(rhs) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_binary_assign_op(
    data: &PluginObjectData,
    op: abi::BinaryOp,
    rhs: &KValue,
    _fn_name: &'static str,
) -> Result<()> {
    let object = data.object_handle();
    let rhs = encode_abi_value(rhs.clone());
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().binary_op_assign)(&HOST_API, object.as_abi(), op, rhs)
    });
    unsafe { value_free(rhs) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

macro_rules! plugin_binary_op {
    ($fn_name:ident, $op:expr, $name:literal) => {
        fn $fn_name(data: &PluginObjectData, other: &KValue) -> Result<KValue> {
            plugin_object_binary_op(data, $op, other, $name)
        }
    };
}

macro_rules! plugin_compare_op {
    ($fn_name:ident, $op:expr, $name:literal) => {
        fn $fn_name(data: &PluginObjectData, other: &KValue) -> Result<bool> {
            match plugin_object_binary_op(data, $op, other, $name)? {
                KValue::Bool(result) => Ok(result),
                unexpected => crate::unexpected_type("Bool", &unexpected),
            }
        }
    };
}

macro_rules! plugin_binary_assign_op {
    ($fn_name:ident, $op:expr, $name:literal) => {
        fn $fn_name(data: &PluginObjectData, other: &KValue) -> Result<()> {
            plugin_object_binary_assign_op(data, $op, other, $name)
        }
    };
}

plugin_binary_op!(plugin_object_add, abi::BinaryOp::Add, "@+");
plugin_binary_op!(plugin_object_add_rhs, abi::BinaryOp::AddRhs, "@+");
plugin_binary_op!(plugin_object_subtract, abi::BinaryOp::Subtract, "@-");
plugin_binary_op!(plugin_object_subtract_rhs, abi::BinaryOp::SubtractRhs, "@-");
plugin_binary_op!(plugin_object_multiply, abi::BinaryOp::Multiply, "@*");
plugin_binary_op!(plugin_object_multiply_rhs, abi::BinaryOp::MultiplyRhs, "@*");
plugin_binary_op!(plugin_object_divide, abi::BinaryOp::Divide, "@/");
plugin_binary_op!(plugin_object_divide_rhs, abi::BinaryOp::DivideRhs, "@/");
plugin_binary_op!(plugin_object_remainder, abi::BinaryOp::Remainder, "@%");
plugin_binary_op!(
    plugin_object_remainder_rhs,
    abi::BinaryOp::RemainderRhs,
    "@%"
);
plugin_binary_op!(plugin_object_power, abi::BinaryOp::Power, "@^");
plugin_binary_op!(plugin_object_power_rhs, abi::BinaryOp::PowerRhs, "@^");

plugin_binary_assign_op!(plugin_object_add_assign, abi::BinaryOp::AddAssign, "@+=");
plugin_binary_assign_op!(
    plugin_object_subtract_assign,
    abi::BinaryOp::SubtractAssign,
    "@-="
);
plugin_binary_assign_op!(
    plugin_object_multiply_assign,
    abi::BinaryOp::MultiplyAssign,
    "@*="
);
plugin_binary_assign_op!(
    plugin_object_divide_assign,
    abi::BinaryOp::DivideAssign,
    "@/="
);
plugin_binary_assign_op!(
    plugin_object_remainder_assign,
    abi::BinaryOp::RemainderAssign,
    "@%="
);
plugin_binary_assign_op!(
    plugin_object_power_assign,
    abi::BinaryOp::PowerAssign,
    "@^="
);

plugin_compare_op!(plugin_object_less, abi::BinaryOp::Less, "@<");
plugin_compare_op!(
    plugin_object_less_or_equal,
    abi::BinaryOp::LessOrEqual,
    "@<="
);
plugin_compare_op!(plugin_object_greater, abi::BinaryOp::Greater, "@>");
plugin_compare_op!(
    plugin_object_greater_or_equal,
    abi::BinaryOp::GreaterOrEqual,
    "@>="
);
plugin_compare_op!(plugin_object_not_equal, abi::BinaryOp::NotEqual, "@!=");

fn plugin_object_equal(data: &PluginObjectData, other: &KValue) -> Result<bool> {
    let object = data.object_handle();
    let other = encode_abi_value(other.clone());
    let mut out = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().equal)(&HOST_API, object.as_abi(), other, &mut out)
    });
    unsafe { value_free(other) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(out)
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_is_iterable(data: &PluginObjectData) -> Result<IsIterable> {
    let object = data.object_handle();
    use abi::IterableKind as AbiKind;

    Ok(
        match with_current_library(data.library.clone(), || unsafe {
            (data.object_v1().iterable_kind)(&HOST_API, object.as_abi())
        }) {
            AbiKind::NotIterable => IsIterable::NotIterable,
            AbiKind::Iterable => IsIterable::Iterable,
            AbiKind::ForwardIterator => IsIterable::ForwardIterator,
            AbiKind::BidirectionalIterator => IsIterable::BidirectionalIterator,
        },
    )
}

fn plugin_object_make_iterator(data: &PluginObjectData, vm: &mut KotoVm) -> Result<KIterator> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().make_iterator)(&HOST_API, object.as_abi(), &mut out)
    });

    if status.code == abi::KotoStatusCode::Ok {
        vm.make_iterator(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn plugin_object_iterator_next(
    data: &PluginObjectData,
    _vm: &mut KotoVm,
) -> Result<Option<KIteratorOutput>> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let mut has_value = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().iterator_next)(&HOST_API, object.as_abi(), &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Ok(Some(KIteratorOutput::Error(status_to_error(status))))
    } else if has_value {
        Ok(Some(KIteratorOutput::Value(take_value(out))))
    } else {
        Ok(None)
    }
}

fn plugin_object_iterator_next_back(
    data: &PluginObjectData,
    _vm: &mut KotoVm,
) -> Result<Option<KIteratorOutput>> {
    let object = data.object_handle();
    let mut out = abi::KValue::default();
    let mut has_value = false;
    let status = with_current_library(data.library.clone(), || unsafe {
        (data.object_v1().iterator_next_back)(&HOST_API, object.as_abi(), &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Ok(Some(KIteratorOutput::Error(status_to_error(status))))
    } else if has_value {
        Ok(Some(KIteratorOutput::Value(take_value(out))))
    } else {
        Ok(None)
    }
}

impl KotoType<RuntimeBackend> for PluginObjectData {
    fn type_string(&self) -> KString {
        self.type_name()
    }
}

impl KotoCopy<RuntimeBackend> for PluginObjectData {
    fn copy(&self) -> KObject {
        make_plugin_object(self.object_v1, self.storage.clone(), self.library.clone())
    }
}

impl KotoAccess<RuntimeBackend> for PluginObjectData {
    fn access(&self, key: &KString) -> Result<Option<KValue>> {
        plugin_object_access(self, key)
    }

    fn access_assign(&mut self, key: &KString, value: &KValue) -> Result<()> {
        plugin_object_access_assign(self, key, value)
    }
}

impl KotoObjectOps<RuntimeBackend> for PluginObjectData {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        plugin_object_display(self, ctx)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        plugin_object_index(self, index)
    }

    fn index_assign(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        plugin_object_index_assign(self, index, value)
    }

    fn size(&self) -> Result<Option<usize>> {
        plugin_object_size(self)
    }

    fn is_callable(&self) -> Result<bool> {
        plugin_object_is_callable(self)
    }

    fn call(&mut self, ctx: &mut CallContext) -> Result<KValue> {
        plugin_object_call(self, ctx)
    }

    fn negate(&self) -> Result<KValue> {
        plugin_object_negate(self)
    }

    fn add(&self, other: &KValue) -> Result<KValue> {
        plugin_object_add(self, other)
    }
    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_add_rhs(self, other)
    }
    fn subtract(&self, other: &KValue) -> Result<KValue> {
        plugin_object_subtract(self, other)
    }
    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_subtract_rhs(self, other)
    }
    fn multiply(&self, other: &KValue) -> Result<KValue> {
        plugin_object_multiply(self, other)
    }
    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_multiply_rhs(self, other)
    }
    fn divide(&self, other: &KValue) -> Result<KValue> {
        plugin_object_divide(self, other)
    }
    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_divide_rhs(self, other)
    }
    fn remainder(&self, other: &KValue) -> Result<KValue> {
        plugin_object_remainder(self, other)
    }
    fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_remainder_rhs(self, other)
    }
    fn power(&self, other: &KValue) -> Result<KValue> {
        plugin_object_power(self, other)
    }
    fn power_rhs(&self, other: &KValue) -> Result<KValue> {
        plugin_object_power_rhs(self, other)
    }
    fn add_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_add_assign(self, other)
    }
    fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_subtract_assign(self, other)
    }
    fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_multiply_assign(self, other)
    }
    fn divide_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_divide_assign(self, other)
    }
    fn remainder_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_remainder_assign(self, other)
    }
    fn power_assign(&mut self, other: &KValue) -> Result<()> {
        plugin_object_power_assign(self, other)
    }
    fn less(&self, other: &KValue) -> Result<bool> {
        plugin_object_less(self, other)
    }
    fn less_or_equal(&self, other: &KValue) -> Result<bool> {
        plugin_object_less_or_equal(self, other)
    }
    fn greater(&self, other: &KValue) -> Result<bool> {
        plugin_object_greater(self, other)
    }
    fn greater_or_equal(&self, other: &KValue) -> Result<bool> {
        plugin_object_greater_or_equal(self, other)
    }
    fn equal(&self, other: &KValue) -> Result<bool> {
        plugin_object_equal(self, other)
    }
    fn not_equal(&self, other: &KValue) -> Result<bool> {
        plugin_object_not_equal(self, other)
    }
    fn is_iterable(&self) -> Result<IsIterable> {
        plugin_object_is_iterable(self)
    }
    fn make_iterator(&self, vm: &mut KotoVm) -> Result<KIterator> {
        plugin_object_make_iterator(self, vm)
    }
    fn iterator_next(&mut self, vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        plugin_object_iterator_next(self, vm)
    }
    fn iterator_next_back(&mut self, vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        plugin_object_iterator_next_back(self, vm)
    }

    fn serialize(&self) -> Result<KValue> {
        Ok(self.copy().into())
    }
}
