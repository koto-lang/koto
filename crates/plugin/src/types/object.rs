use crate::{
    CallContext, DisplayContext, KIterator, KIteratorOutput, KList, KMap, KNumber, KRange, KString,
    KTuple, KValue, KotoSend, KotoSync, KotoVm, Result,
    api::{BinaryOp, UnaryOp},
    error::{Error, unexpected_type},
    host::{current_host_api, status_to_error, string_slice, with_host_api},
    runtime_error,
    types::{decode_value, encode_value},
    vm::{abi_binary_op, abi_unary_op},
};
use koto_api::{
    KotoAccess, KotoBackend, KotoCopy, KotoIdentity, KotoMethodContext, KotoNamedAccess,
    KotoObjectCast, KotoObjectHandle, KotoObjectOps, KotoType,
};
use koto_ffi as abi;
use std::{
    any::Any,
    collections::HashMap,
    ffi::c_void,
    marker::PhantomData,
    mem::{ManuallyDrop, size_of},
    ops::{Deref, DerefMut},
    panic::{AssertUnwindSafe, catch_unwind},
    slice,
    sync::{Mutex, OnceLock},
};

fn object_unary_op<T: KotoObject>(handle: &T, op: UnaryOp) -> Result<KValue> {
    match op {
        UnaryOp::Negate => handle.negate(),
        _ => runtime_error!("unsupported plugin object unary op: {op:?}"),
    }
}

fn object_binary_op<T: KotoObject>(handle: &T, op: BinaryOp, rhs: &KValue) -> Result<KValue> {
    match op {
        BinaryOp::Add => handle.add(rhs),
        BinaryOp::AddRhs => handle.add_rhs(rhs),
        BinaryOp::Subtract => handle.subtract(rhs),
        BinaryOp::SubtractRhs => handle.subtract_rhs(rhs),
        BinaryOp::Multiply => handle.multiply(rhs),
        BinaryOp::MultiplyRhs => handle.multiply_rhs(rhs),
        BinaryOp::Divide => handle.divide(rhs),
        BinaryOp::DivideRhs => handle.divide_rhs(rhs),
        BinaryOp::Remainder => handle.remainder(rhs),
        BinaryOp::RemainderRhs => handle.remainder_rhs(rhs),
        BinaryOp::Power => handle.power(rhs),
        BinaryOp::PowerRhs => handle.power_rhs(rhs),
        BinaryOp::Less => handle.less(rhs).map(Into::into),
        BinaryOp::LessOrEqual => handle.less_or_equal(rhs).map(Into::into),
        BinaryOp::Greater => handle.greater(rhs).map(Into::into),
        BinaryOp::GreaterOrEqual => handle.greater_or_equal(rhs).map(Into::into),
        BinaryOp::NotEqual => handle.not_equal(rhs).map(Into::into),
        _ => runtime_error!("unsupported plugin object binary op: {op:?}"),
    }
}

fn object_binary_op_assign<T: KotoObject>(
    handle: &mut T,
    op: BinaryOp,
    rhs: &KValue,
) -> Result<()> {
    match op {
        BinaryOp::AddAssign => handle.add_assign(rhs),
        BinaryOp::SubtractAssign => handle.subtract_assign(rhs),
        BinaryOp::MultiplyAssign => handle.multiply_assign(rhs),
        BinaryOp::DivideAssign => handle.divide_assign(rhs),
        BinaryOp::RemainderAssign => handle.remainder_assign(rhs),
        BinaryOp::PowerAssign => handle.power_assign(rhs),
        _ => runtime_error!("unsupported plugin object assign op: {op:?}"),
    }
}

/// An owning handle to a runtime object value.
pub struct KObject {
    api: *const abi::KotoHostApiV1,
    handle: abi::KObject,
}

unsafe impl Send for KObject {}

unsafe impl Sync for KObject {}

/// An immutable borrow of runtime-owned plugin object data.
pub struct Borrow<'a, T> {
    api: *const abi::KotoHostApiV1,
    handle: abi::KObjectBorrow,
    ptr: *const T,
    _phantom: PhantomData<&'a T>,
}

impl<T> Deref for Borrow<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> Drop for Borrow<'_, T> {
    fn drop(&mut self) {
        unsafe { ((*self.api).object_borrow_free)(self.handle) };
    }
}

/// A mutable borrow of runtime-owned plugin object data.
pub struct BorrowMut<'a, T> {
    api: *const abi::KotoHostApiV1,
    handle: abi::KObjectBorrowMut,
    ptr: *mut T,
    _phantom: PhantomData<&'a mut T>,
}

impl<T> Deref for BorrowMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for BorrowMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for BorrowMut<'_, T> {
    fn drop(&mut self) {
        unsafe { ((*self.api).object_borrow_mut_free)(self.handle) };
    }
}

/// The plugin backend marker used by [`KotoObjectOps`].
pub struct PluginBackend;

impl KotoBackend for PluginBackend {
    type Error = crate::Error;
    type Value = KValue;
    type Number = KNumber;
    type Range = KRange;
    type String = KString;
    type List = KList;
    type Tuple = KTuple;
    type Map = KMap;
    type Object = KObject;
    type Iterator = KIterator;
    type IteratorOutput = KIteratorOutput;
    type Function = crate::KFunction;
    type NativeFunction = crate::KNativeFunction;
    type Vm = KotoVm;
    type DisplayContext<'a>
        = DisplayContext<'a>
    where
        Self: 'a;
    type CallContext<'a>
        = CallContext
    where
        Self: 'a;

    fn unimplemented_object_op<T>(
        op: &'static str,
        object_type: Self::String,
    ) -> std::result::Result<T, Self::Error> {
        runtime_error!(format!("{op} is unimplemented for {object_type}"))
    }

    fn is_unimplemented_error(error: &Self::Error) -> bool {
        error.is_unimplemented_error()
    }
}

/// An immutable borrowed view of a [`KObject`]'s behavior.
pub struct ObjectBorrow<'a> {
    api: *const abi::KotoHostApiV1,
    handle: abi::KObjectBorrow,
    _phantom: PhantomData<&'a KObject>,
}

/// A mutable borrowed view of a [`KObject`]'s behavior.
pub struct ObjectBorrowMut<'a> {
    api: *const abi::KotoHostApiV1,
    handle: abi::KObjectBorrowMut,
    _phantom: PhantomData<&'a mut KObject>,
}

impl<'a> ObjectBorrowMut<'a> {
    fn with_shared<R>(&self, f: impl FnOnce(&ObjectBorrow<'a>) -> R) -> R {
        let shared = ManuallyDrop::new(ObjectBorrow {
            api: self.api,
            handle: self.handle.as_shared(),
            _phantom: PhantomData,
        });
        let shared_ref = unsafe {
            &*(&shared as *const ManuallyDrop<ObjectBorrow<'a>> as *const ObjectBorrow<'a>)
        };

        f(shared_ref)
    }
}

impl Drop for ObjectBorrow<'_> {
    fn drop(&mut self) {
        unsafe { ((*self.api).object_borrow_free)(self.handle) };
    }
}

impl Drop for ObjectBorrowMut<'_> {
    fn drop(&mut self) {
        unsafe { ((*self.api).object_borrow_mut_free)(self.handle) };
    }
}

fn borrowed_object_type_string(api: &abi::KotoHostApiV1, borrow: abi::KObjectBorrow) -> KString {
    KString::from_handle(api, unsafe { (api.object_borrow_type_string)(borrow) })
}

fn borrowed_object_named_value(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
    key: &str,
) -> Result<Option<KValue>> {
    let mut out = abi::KValue::default();
    let mut found = false;
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_named_value)(borrow, string_slice(key), &mut out, &mut found)
    });

    if status.code != abi::KotoStatusCode::Ok {
        return Err(status_to_error(status));
    }

    if found {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result.map(Some)
    } else {
        Ok(None)
    }
}

fn borrowed_object_named_value_assign(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
    key: &str,
    value: &KValue,
) -> Result<()> {
    let value = encode_value(api, value.clone());
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_named_value_assign)(borrow, string_slice(key), value)
    });

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

fn borrowed_object_display(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> Result<KString> {
    let string = with_host_api(api, || unsafe { (api.object_borrow_display)(borrow) });
    Ok(KString::from_handle(api, string))
}

fn borrowed_object_iterable_kind(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> IsIterable {
    match with_host_api(api, || unsafe { (api.object_borrow_iterable_kind)(borrow) }) {
        abi::IterableKind::NotIterable => IsIterable::NotIterable,
        abi::IterableKind::Iterable => IsIterable::Iterable,
        abi::IterableKind::ForwardIterator => IsIterable::ForwardIterator,
        abi::IterableKind::BidirectionalIterator => IsIterable::BidirectionalIterator,
    }
}

fn borrowed_object_make_iterator(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> Result<KIterator> {
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_make_iterator)(borrow, &mut out)
    });

    if status.code != abi::KotoStatusCode::Ok {
        return Err(status_to_error(status));
    }

    let result = decode_value(api, out);
    unsafe { (api.value_free)(out) };

    match result? {
        KValue::Iterator(iterator) => Ok(iterator),
        unexpected => unexpected_type("Iterator", &unexpected),
    }
}

fn borrowed_object_iterator_next(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
) -> Result<Option<KIteratorOutput>> {
    let mut out = abi::KValue::default();
    let mut has_value = false;
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_iterator_next)(borrow, &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Ok(Some(KIteratorOutput::Error(status_to_error(status))))
    } else if has_value {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result.map(KIteratorOutput::Value).map(Some)
    } else {
        Ok(None)
    }
}

fn borrowed_object_iterator_next_back(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
) -> Result<Option<KIteratorOutput>> {
    let mut out = abi::KValue::default();
    let mut has_value = false;
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_iterator_next_back)(borrow, &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Ok(Some(KIteratorOutput::Error(status_to_error(status))))
    } else if has_value {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result.map(KIteratorOutput::Value).map(Some)
    } else {
        Ok(None)
    }
}

fn borrowed_object_size(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> Result<Option<usize>> {
    let mut out = 0;
    let mut has_value = false;
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_size)(borrow, &mut out, &mut has_value)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else if has_value {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

fn borrowed_object_index(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
    index: &KValue,
) -> Result<KValue> {
    let index = encode_value(api, index.clone());
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_index)(borrow, index, &mut out)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result
    }
}

fn borrowed_object_index_assign(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
    index: &KValue,
    value: &KValue,
) -> Result<()> {
    let index = encode_value(api, index.clone());
    let value = encode_value(api, value.clone());
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_index_assign)(borrow, index, value)
    });

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

fn borrowed_object_is_callable(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> Result<bool> {
    let mut out = false;
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_is_callable)(borrow, &mut out)
    });

    if status.code == abi::KotoStatusCode::Ok {
        Ok(out)
    } else {
        Err(status_to_error(status))
    }
}

fn borrowed_object_call(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
    ctx: &mut CallContext,
) -> Result<KValue> {
    let instance = encode_value(api, ctx.instance().clone());
    let args = ctx
        .args()
        .iter()
        .cloned()
        .map(|arg| encode_value(api, arg))
        .collect::<Vec<_>>();
    let abi_ctx = abi::CallContext {
        instance,
        args: args.as_ptr(),
        arg_count: args.len(),
    };
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_call)(borrow, abi_ctx, &mut out)
    });

    unsafe { (api.value_free)(instance) };
    for arg in args {
        unsafe { (api.value_free)(arg) };
    }

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result
    }
}

fn borrowed_object_unary_op_value(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
    op: UnaryOp,
) -> Result<KValue> {
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_unary_op)(borrow, abi_unary_op(op), &mut out)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result
    }
}

fn borrowed_object_binary_op_value(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
    op: BinaryOp,
    rhs: &KValue,
) -> Result<KValue> {
    let rhs = encode_value(api, rhs.clone());
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_binary_op)(borrow, abi_binary_op(op), rhs, &mut out)
    });

    unsafe { (api.value_free)(rhs) };

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result
    }
}

fn borrowed_object_binary_op_bool(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
    op: BinaryOp,
    rhs: &KValue,
) -> Result<bool> {
    match borrowed_object_binary_op_value(api, borrow, op, rhs)? {
        KValue::Bool(result) => Ok(result),
        unexpected => unexpected_type("Bool", &unexpected),
    }
}

fn borrowed_object_binary_op_assign(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrowMut,
    op: BinaryOp,
    rhs: &KValue,
) -> Result<()> {
    let rhs = encode_value(api, rhs.clone());
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_binary_op_assign)(borrow, abi_binary_op(op), rhs)
    });

    unsafe { (api.value_free)(rhs) };

    if status.code == abi::KotoStatusCode::Ok {
        Ok(())
    } else {
        Err(status_to_error(status))
    }
}

fn borrowed_object_serialize(
    api: &abi::KotoHostApiV1,
    borrow: abi::KObjectBorrow,
) -> Result<KValue> {
    let mut out = abi::KValue::default();
    let status = with_host_api(api, || unsafe {
        (api.object_borrow_serialize)(borrow, &mut out)
    });

    if status.code != abi::KotoStatusCode::Ok {
        Err(status_to_error(status))
    } else {
        let result = decode_value(api, out);
        unsafe { (api.value_free)(out) };
        result
    }
}

impl KObject {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        debug_assert!(matches!(handle.kind, abi::KValueKind::Object));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.object_value },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: this.handle,
            },
        }
    }

    fn as_value(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: self.handle,
            },
        }
    }

    fn with_object_handle<T>(
        &self,
        f: impl FnOnce(&abi::KotoHostApiV1, &'static abi::KotoPluginObjectV1, abi::KObject) -> T,
    ) -> Option<T> {
        let api = unsafe { &*self.api };

        let object_v1 = unsafe { (api.object_v1)(self.handle) };
        if object_v1.is_null() {
            return None;
        }

        Some(f(api, unsafe { &*object_v1 }, self.handle))
    }

    /// Borrows the object's behavior immutably.
    pub fn try_borrow(&self) -> Result<ObjectBorrow<'_>> {
        let api = unsafe { &*self.api };
        let handle = unsafe { (api.object_borrow)(self.handle) };

        if handle.is_valid() {
            Ok(ObjectBorrow {
                api: self.api,
                handle,
                _phantom: PhantomData,
            })
        } else {
            Err(Error::new("unable to borrow object"))
        }
    }

    /// Borrows the object's behavior mutably.
    pub fn try_borrow_mut(&self) -> Result<ObjectBorrowMut<'_>> {
        let api = unsafe { &*self.api };
        let handle = unsafe { (api.object_borrow_mut)(self.handle) };

        if handle.is_valid() {
            Ok(ObjectBorrowMut {
                api: self.api,
                handle,
                _phantom: PhantomData,
            })
        } else {
            Err(Error::new("unable to mutably borrow object"))
        }
    }

    /// Returns true if the object is of the given Rust type.
    pub fn is_a<T: KotoType<PluginBackend> + 'static>(&self) -> bool {
        self.with_object_handle(|_, object_v1, _| object_v1.type_tag == type_tag::<T>())
            .unwrap_or(false)
    }

    /// Attempts to borrow and cast the object to the specified Rust type.
    pub fn cast<T: KotoType<PluginBackend> + 'static>(&self) -> Result<Borrow<'_, T>> {
        self.with_object_handle(|api, object_v1, instance| {
            if object_v1.type_tag != type_tag::<T>() {
                let unexpected = KString::from_handle(api, unsafe {
                    (api.string_make)((object_v1.type_string)(api, instance))
                });
                return Err(Error::new(format!(
                    "expected {}, found {}",
                    T::type_static(),
                    unexpected
                )));
            }

            let handle = unsafe { (api.object_borrow)(instance) };
            if !handle.is_valid() {
                return Err(Error::new("unable to borrow object"));
            }

            let ptr = handle.data as *const T;
            if ptr.is_null() {
                unsafe { (api.object_borrow_free)(handle) };
                return Err(Error::new("unable to borrow object"));
            }

            Ok(Borrow {
                api: api as *const _,
                handle,
                ptr,
                _phantom: PhantomData,
            })
        })
        .unwrap_or_else(|| Err(Error::new("unable to borrow object")))
    }

    /// Attempts to mutably borrow and cast the object to the specified Rust type.
    pub fn cast_mut<T: KotoType<PluginBackend> + 'static>(&self) -> Result<BorrowMut<'_, T>> {
        self.with_object_handle(|api, object_v1, instance| {
            if object_v1.type_tag != type_tag::<T>() {
                let unexpected = KString::from_handle(api, unsafe {
                    (api.string_make)((object_v1.type_string)(api, instance))
                });
                return Err(Error::new(format!(
                    "expected {}, found {}",
                    T::type_static(),
                    unexpected
                )));
            }

            let handle = unsafe { (api.object_borrow_mut)(instance) };
            if !handle.is_valid() {
                return Err(Error::new("unable to mutably borrow object"));
            }

            let ptr = handle.data as *mut T;
            if ptr.is_null() {
                unsafe { (api.object_borrow_mut_free)(handle) };
                return Err(Error::new("unable to mutably borrow object"));
            }

            Ok(BorrowMut {
                api: api as *const _,
                handle,
                ptr,
                _phantom: PhantomData,
            })
        })
        .unwrap_or_else(|| Err(Error::new("unable to mutably borrow object")))
    }

    /// Returns `true` if both objects refer to the same underlying runtime instance.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        let api = unsafe { &*self.api };
        std::ptr::eq(api, unsafe { &*other.api })
            && unsafe { (api.value_is_same_instance)(self.as_value(), other.as_value()) }
    }
}

impl Clone for KObject {
    fn clone(&self) -> Self {
        let api = unsafe { &*self.api };
        let cloned = unsafe { (api.value_clone)(self.as_value()) };
        Self {
            api: self.api,
            handle: unsafe { cloned.data.object_value },
        }
    }
}

impl Drop for KObject {
    fn drop(&mut self) {
        let api = unsafe { &*self.api };
        unsafe { (api.value_free)(self.as_value()) };
    }
}

impl std::fmt::Debug for KObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KObject")
    }
}

impl KotoObjectHandle<PluginBackend> for ObjectBorrow<'_> {
    fn type_string(&self) -> KString {
        borrowed_object_type_string(unsafe { &*self.api }, self.handle)
    }
}

impl KotoObjectHandle<PluginBackend> for ObjectBorrowMut<'_> {
    fn type_string(&self) -> KString {
        self.with_shared(|shared| shared.type_string())
    }
}

impl KotoNamedAccess<PluginBackend> for ObjectBorrow<'_> {
    fn named_value(&self, key: &str) -> Result<Option<KValue>> {
        borrowed_object_named_value(unsafe { &*self.api }, self.handle, key)
    }

    fn named_value_assign(&mut self, key: &str, value: &KValue) -> Result<()> {
        let _ = (key, value);
        Err(Error::new("object borrow is immutable"))
    }
}

impl KotoNamedAccess<PluginBackend> for ObjectBorrowMut<'_> {
    fn named_value(&self, key: &str) -> Result<Option<KValue>> {
        self.with_shared(|shared| shared.named_value(key))
    }

    fn named_value_assign(&mut self, key: &str, value: &KValue) -> Result<()> {
        borrowed_object_named_value_assign(unsafe { &*self.api }, self.handle, key, value)
    }
}

impl KotoObjectOps<PluginBackend> for ObjectBorrow<'_> {
    fn display(&self, ctx: &mut DisplayContext<'_>) -> Result<()> {
        ctx.append(borrowed_object_display(unsafe { &*self.api }, self.handle)?);
        Ok(())
    }

    fn negate(&self) -> Result<KValue> {
        borrowed_object_unary_op_value(unsafe { &*self.api }, self.handle, UnaryOp::Negate)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        borrowed_object_index(unsafe { &*self.api }, self.handle, index)
    }

    fn size(&self) -> Result<Option<usize>> {
        borrowed_object_size(unsafe { &*self.api }, self.handle)
    }

    fn is_callable(&self) -> Result<bool> {
        borrowed_object_is_callable(unsafe { &*self.api }, self.handle)
    }

    fn is_iterable(&self) -> Result<IsIterable> {
        Ok(borrowed_object_iterable_kind(
            unsafe { &*self.api },
            self.handle,
        ))
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        borrowed_object_make_iterator(unsafe { &*self.api }, self.handle)
    }

    fn add(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(unsafe { &*self.api }, self.handle, BinaryOp::Add, other)
    }

    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(unsafe { &*self.api }, self.handle, BinaryOp::AddRhs, other)
    }

    fn subtract(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::Subtract,
            other,
        )
    }

    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::SubtractRhs,
            other,
        )
    }

    fn multiply(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::Multiply,
            other,
        )
    }

    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::MultiplyRhs,
            other,
        )
    }

    fn divide(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(unsafe { &*self.api }, self.handle, BinaryOp::Divide, other)
    }

    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::DivideRhs,
            other,
        )
    }

    fn remainder(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::Remainder,
            other,
        )
    }

    fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::RemainderRhs,
            other,
        )
    }

    fn power(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(unsafe { &*self.api }, self.handle, BinaryOp::Power, other)
    }

    fn power_rhs(&self, other: &KValue) -> Result<KValue> {
        borrowed_object_binary_op_value(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::PowerRhs,
            other,
        )
    }

    fn equal(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(unsafe { &*self.api }, self.handle, BinaryOp::Equal, other)
    }

    fn less(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(unsafe { &*self.api }, self.handle, BinaryOp::Less, other)
    }

    fn less_or_equal(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::LessOrEqual,
            other,
        )
    }

    fn greater(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(unsafe { &*self.api }, self.handle, BinaryOp::Greater, other)
    }

    fn greater_or_equal(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::GreaterOrEqual,
            other,
        )
    }

    fn not_equal(&self, other: &KValue) -> Result<bool> {
        borrowed_object_binary_op_bool(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::NotEqual,
            other,
        )
    }

    fn serialize(&self) -> Result<KValue> {
        borrowed_object_serialize(unsafe { &*self.api }, self.handle)
    }
}

impl KotoObjectOps<PluginBackend> for ObjectBorrowMut<'_> {
    fn display(&self, ctx: &mut DisplayContext<'_>) -> Result<()> {
        self.with_shared(|shared| shared.display(ctx))
    }

    fn negate(&self) -> Result<KValue> {
        self.with_shared(KotoObjectOps::<PluginBackend>::negate)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.index(index))
    }

    fn index_assign(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        borrowed_object_index_assign(unsafe { &*self.api }, self.handle, index, value)
    }

    fn size(&self) -> Result<Option<usize>> {
        self.with_shared(KotoObjectOps::<PluginBackend>::size)
    }

    fn is_callable(&self) -> Result<bool> {
        self.with_shared(KotoObjectOps::<PluginBackend>::is_callable)
    }

    fn call(&mut self, ctx: &mut CallContext) -> Result<KValue> {
        borrowed_object_call(unsafe { &*self.api }, self.handle, ctx)
    }

    fn is_iterable(&self) -> Result<IsIterable> {
        self.with_shared(KotoObjectOps::<PluginBackend>::is_iterable)
    }

    fn make_iterator(&self, vm: &mut KotoVm) -> Result<KIterator> {
        self.with_shared(|shared| shared.make_iterator(vm))
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        borrowed_object_iterator_next(unsafe { &*self.api }, self.handle)
    }

    fn iterator_next_back(&mut self, _vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        borrowed_object_iterator_next_back(unsafe { &*self.api }, self.handle)
    }

    fn add(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.add(other))
    }

    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.add_rhs(other))
    }

    fn subtract(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.subtract(other))
    }

    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.subtract_rhs(other))
    }

    fn multiply(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.multiply(other))
    }

    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.multiply_rhs(other))
    }

    fn divide(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.divide(other))
    }

    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.divide_rhs(other))
    }

    fn remainder(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.remainder(other))
    }

    fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.remainder_rhs(other))
    }

    fn power(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.power(other))
    }

    fn power_rhs(&self, other: &KValue) -> Result<KValue> {
        self.with_shared(|shared| shared.power_rhs(other))
    }

    fn add_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::AddAssign,
            other,
        )
    }

    fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::SubtractAssign,
            other,
        )
    }

    fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::MultiplyAssign,
            other,
        )
    }

    fn divide_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::DivideAssign,
            other,
        )
    }

    fn remainder_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::RemainderAssign,
            other,
        )
    }

    fn power_assign(&mut self, other: &KValue) -> Result<()> {
        borrowed_object_binary_op_assign(
            unsafe { &*self.api },
            self.handle,
            BinaryOp::PowerAssign,
            other,
        )
    }

    fn equal(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.equal(other))
    }

    fn less(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.less(other))
    }

    fn less_or_equal(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.less_or_equal(other))
    }

    fn greater(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.greater(other))
    }

    fn greater_or_equal(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.greater_or_equal(other))
    }

    fn not_equal(&self, other: &KValue) -> Result<bool> {
        self.with_shared(|shared| shared.not_equal(other))
    }

    fn serialize(&self) -> Result<KValue> {
        self.with_shared(KotoObjectOps::<PluginBackend>::serialize)
    }
}

impl KotoIdentity for KObject {
    fn is_same_instance(&self, other: &Self) -> bool {
        KObject::is_same_instance(self, other)
    }
}

impl KotoObjectCast<PluginBackend> for KObject {
    type ObjectRef<'a, T: 'static>
        = Borrow<'a, T>
    where
        Self: 'a;
    type ObjectRefMut<'a, T: 'static>
        = BorrowMut<'a, T>
    where
        Self: 'a;

    fn is_a<T: KotoType<PluginBackend> + 'static>(&self) -> bool {
        KObject::is_a::<T>(self)
    }

    fn cast<T: KotoType<PluginBackend> + 'static>(&self) -> Result<Self::ObjectRef<'_, T>> {
        KObject::cast::<T>(self)
    }

    fn cast_mut<T: KotoType<PluginBackend> + 'static>(
        &mut self,
    ) -> Result<Self::ObjectRefMut<'_, T>> {
        KObject::cast_mut::<T>(self)
    }
}

/// Indicates whether a plugin-owned object is iterable.
pub use koto_api::KotoObjectIterable as IsIterable;

/// A trait that represents the basic requirements of fields in a type that implements
/// [`KotoObject`].
pub trait KotoField: Clone + KotoSend + KotoSync + 'static {}
impl<T> KotoField for T where T: Clone + KotoSend + KotoSync + 'static {}

/// A context provided to a plugin-owned object method.
pub struct MethodContext<'a, T> {
    /// A VM facade backed by the active host callback.
    pub vm: KotoVm,
    object: &'a KObject,
    /// The method call arguments.
    pub args: &'a [KValue],
    _phantom: PhantomData<T>,
}

impl<'a, T: KotoObject> MethodContext<'a, T> {
    /// Makes a new method context.
    pub fn new(object: &'a KObject, args: &'a [KValue], vm: KotoVm) -> Self {
        Self {
            vm,
            object,
            args,
            _phantom: PhantomData,
        }
    }

    /// Returns the method call arguments.
    pub fn args(&self) -> &[KValue] {
        self.args
    }

    /// Returns an immutable reference to the object instance.
    pub fn instance(&self) -> Result<Borrow<'_, T>> {
        self.object.cast::<T>()
    }

    /// Returns a mutable reference to the object instance.
    pub fn instance_mut(&self) -> Result<BorrowMut<'_, T>> {
        self.object.cast_mut::<T>()
    }

    /// Returns a clone of the instance as a Koto value.
    pub fn instance_result(&self) -> Result<KValue> {
        Ok(self.object.clone().into())
    }
}

impl<T: KotoObject> KotoMethodContext<PluginBackend> for MethodContext<'_, T> {
    type Instance<'a>
        = Borrow<'a, T>
    where
        Self: 'a;
    type InstanceMut<'a>
        = BorrowMut<'a, T>
    where
        Self: 'a;

    fn vm(&self) -> &KotoVm {
        &self.vm
    }

    fn args(&self) -> &[KValue] {
        self.args
    }

    fn instance(&self) -> Result<Self::Instance<'_>> {
        MethodContext::instance(self)
    }

    fn instance_mut(&mut self) -> Result<Self::InstanceMut<'_>> {
        MethodContext::instance_mut(self)
    }

    fn instance_result(&self) -> Result<KValue> {
        MethodContext::instance_result(self)
    }
}

/// Trait implemented by Rust types that should be exposed as plugin-owned objects.
pub trait KotoObject:
    KotoObjectOps<PluginBackend>
    + KotoType<PluginBackend>
    + KotoCopy<PluginBackend>
    + KotoAccess<PluginBackend>
    + KotoSend
    + KotoSync
    + Any
{
}

impl<T> KotoObject for T where
    T: KotoObjectOps<PluginBackend>
        + KotoType<PluginBackend>
        + KotoCopy<PluginBackend>
        + KotoAccess<PluginBackend>
        + KotoSend
        + KotoSync
        + Any
{
}

pub(crate) fn make_method_value(method: fn(&mut CallContext) -> Result<KValue>) -> KValue {
    crate::KNativeFunction::new(method).into()
}

fn borrow_object<T>(
    api: &abi::KotoHostApiV1,
    instance: abi::KObject,
) -> Result<Borrow<'static, T>> {
    let handle = unsafe { (api.object_borrow)(instance) };
    if !handle.is_valid() {
        return Err(Error::new("unable to borrow plugin object"));
    }

    let ptr = handle.data as *const T;
    if ptr.is_null() {
        unsafe { (api.object_borrow_free)(handle) };
        return Err(Error::new("unable to borrow plugin object"));
    }

    Ok(Borrow {
        api: api as *const _,
        handle,
        ptr,
        _phantom: PhantomData,
    })
}

fn borrow_object_mut<T>(
    api: &abi::KotoHostApiV1,
    instance: abi::KObject,
) -> Result<BorrowMut<'static, T>> {
    let handle = unsafe { (api.object_borrow_mut)(instance) };
    if !handle.is_valid() {
        return Err(Error::new("unable to mutably borrow plugin object"));
    }

    let ptr = handle.data as *mut T;
    if ptr.is_null() {
        unsafe { (api.object_borrow_mut_free)(handle) };
        return Err(Error::new("unable to mutably borrow plugin object"));
    }

    Ok(BorrowMut {
        api: api as *const _,
        handle,
        ptr,
        _phantom: PhantomData,
    })
}

fn type_tag<T>() -> usize {
    type_tag_impl::<T> as *const () as usize
}

fn type_tag_impl<T>() {}

fn rust_object_v1<T>() -> &'static abi::KotoPluginObjectV1
where
    T: KotoObject + 'static,
{
    static REGISTRY: OnceLock<Mutex<HashMap<usize, &'static abi::KotoPluginObjectV1>>> =
        OnceLock::new();

    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let key = type_tag::<T>();

    let mut registry = registry
        .lock()
        .expect("plugin object registry was poisoned");
    registry.entry(key).or_insert_with(|| {
        Box::leak(Box::new(abi::KotoPluginObjectV1 {
            struct_size: size_of::<abi::KotoPluginObjectV1>(),
            type_tag: type_tag::<T>(),
            type_string: object_type_string_trampoline::<T>,
            named_value: object_named_value_trampoline::<T>,
            iterable_kind: object_iterable_kind_trampoline::<T>,
            iterator_next: object_iterator_next_trampoline::<T>,
            iterator_next_back: object_iterator_next_back_trampoline::<T>,
            named_value_assign: object_named_value_assign_trampoline::<T>,
            call: object_call_trampoline::<T>,
            display: object_display_trampoline::<T>,
            size: object_size_trampoline::<T>,
            is_callable: object_is_callable_trampoline::<T>,
            index: object_index_trampoline::<T>,
            index_assign: object_index_assign_trampoline::<T>,
            equal: object_equal_trampoline::<T>,
            unary_op: object_unary_op_trampoline::<T>,
            binary_op: object_binary_op_trampoline::<T>,
            binary_op_assign: object_binary_op_assign_trampoline::<T>,
            make_iterator: object_make_iterator_trampoline::<T>,
        }))
    })
}

unsafe extern "C" fn object_init_trampoline<T>(storage: *mut c_void, source: *mut c_void) {
    unsafe {
        (storage as *mut T).write(std::ptr::read(source as *const T));
    }
}

unsafe extern "C" fn object_drop_data_trampoline<T>(storage: *mut c_void) {
    unsafe {
        std::ptr::drop_in_place(storage as *mut T);
    }
}

unsafe extern "C" fn object_type_string_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
) -> abi::KStringSlice
where
    T: KotoObject,
{
    let api = unsafe { &*host_api };
    match borrow_object::<T>(api, instance) {
        Ok(_object) => string_slice(T::type_static()),
        Err(_) => abi::KStringSlice::default(),
    }
}

unsafe extern "C" fn object_named_value_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    key: abi::KStringSlice,
    out: *mut abi::KValue,
    out_found: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let key = KString::from_slice(api, key);
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };

            match object.access(&key) {
                Ok(Some(value)) => {
                    unsafe {
                        *out = encode_value(api, value);
                        *out_found = true;
                    }
                    abi::KotoStatus::ok()
                }
                Ok(None) => {
                    unsafe { *out_found = false };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_named_value_assign_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    key: abi::KStringSlice,
    value: abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let key = KString::from_slice(api, key);
            let value = match decode_value(api, value) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };

            match object.access_assign(&key, &value) {
                Ok(()) => abi::KotoStatus::ok(),
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_call_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    ctx: abi::CallContext,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let arg_handles = unsafe { slice::from_raw_parts(ctx.args, ctx.arg_count) };
            for arg in arg_handles {
                if matches!(arg.kind, abi::KValueKind::Unsupported) {
                    return Error::new("unsupported runtime value for plugin ABI v1").into_status();
                }
            }

            let instance_value = match decode_value(api, ctx.instance) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };

            let mut ctx = CallContext::from_abi(
                api,
                KotoVm::from_api(api),
                instance_value,
                ctx.args,
                ctx.arg_count,
            );
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };

            match object.call(&mut ctx) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_display_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let vm = KotoVm::from_api(api);
            let mut display_context = DisplayContext::with_vm(&vm);
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            match object.display(&mut display_context) {
                Ok(()) => {
                    unsafe { *out = encode_value(api, KValue::from(display_context.result())) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_unary_op_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    op: abi::UnaryOp,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let op = match op {
                abi::UnaryOp::Negate => UnaryOp::Negate,
                _ => return Error::new("unsupported plugin object unary op").into_status(),
            };

            match object_unary_op(&*object, op) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_binary_op_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    op: abi::BinaryOp,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let rhs = match decode_value(api, rhs) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };
            let op = match op {
                abi::BinaryOp::Add => BinaryOp::Add,
                abi::BinaryOp::Subtract => BinaryOp::Subtract,
                abi::BinaryOp::Multiply => BinaryOp::Multiply,
                abi::BinaryOp::Divide => BinaryOp::Divide,
                abi::BinaryOp::Remainder => BinaryOp::Remainder,
                abi::BinaryOp::Power => BinaryOp::Power,
                abi::BinaryOp::AddRhs => BinaryOp::AddRhs,
                abi::BinaryOp::SubtractRhs => BinaryOp::SubtractRhs,
                abi::BinaryOp::MultiplyRhs => BinaryOp::MultiplyRhs,
                abi::BinaryOp::DivideRhs => BinaryOp::DivideRhs,
                abi::BinaryOp::RemainderRhs => BinaryOp::RemainderRhs,
                abi::BinaryOp::PowerRhs => BinaryOp::PowerRhs,
                _ => return Error::new("unsupported plugin object binary op").into_status(),
            };

            match object_binary_op(&*object, op, &rhs) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_binary_op_assign_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    op: abi::BinaryOp,
    rhs: abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let rhs = match decode_value(api, rhs) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };
            let op = match op {
                abi::BinaryOp::AddAssign => BinaryOp::AddAssign,
                abi::BinaryOp::SubtractAssign => BinaryOp::SubtractAssign,
                abi::BinaryOp::MultiplyAssign => BinaryOp::MultiplyAssign,
                abi::BinaryOp::DivideAssign => BinaryOp::DivideAssign,
                abi::BinaryOp::RemainderAssign => BinaryOp::RemainderAssign,
                abi::BinaryOp::PowerAssign => BinaryOp::PowerAssign,
                _ => return Error::new("unsupported plugin object assign op").into_status(),
            };

            match object_binary_op_assign(&mut *object, op, &rhs) {
                Ok(()) => abi::KotoStatus::ok(),
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_size_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut usize,
    out_has_value: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        let object = match borrow_object::<T>(api, instance) {
            Ok(object) => object,
            Err(error) => return error.into_status(),
        };
        match object.size() {
            Ok(Some(size)) => unsafe {
                *out = size;
                *out_has_value = true;
                abi::KotoStatus::ok()
            },
            Ok(None) => unsafe {
                *out_has_value = false;
                abi::KotoStatus::ok()
            },
            Err(error) => error.into_status(),
        }
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_is_callable_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        let object = match borrow_object::<T>(api, instance) {
            Ok(object) => object,
            Err(error) => return error.into_status(),
        };

        match object.is_callable() {
            Ok(is_callable) => unsafe {
                *out = is_callable;
                abi::KotoStatus::ok()
            },
            Err(error) => error.into_status(),
        }
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_index_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    index: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let index = match decode_value(api, index) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };

            match object.index(&index) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_index_assign_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    index: abi::KValue,
    value: abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let index = match decode_value(api, index) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };
            let value = match decode_value(api, value) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };

            match object.index_assign(&index, &value) {
                Ok(()) => abi::KotoStatus::ok(),
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_equal_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    other: abi::KValue,
    out: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            let other = match decode_value(api, other) {
                Ok(value) => value,
                Err(error) => return error.into_status(),
            };

            match object.equal(&other) {
                Ok(result) => unsafe {
                    *out = result;
                    abi::KotoStatus::ok()
                },
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_iterable_kind_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
) -> abi::IterableKind
where
    T: KotoObject,
{
    let api = unsafe { &*host_api };
    let Ok(object) = borrow_object::<T>(api, instance) else {
        return abi::IterableKind::NotIterable;
    };
    match object.is_iterable() {
        Ok(IsIterable::NotIterable) => abi::IterableKind::NotIterable,
        Ok(IsIterable::Iterable) => abi::IterableKind::Iterable,
        Ok(IsIterable::ForwardIterator) => abi::IterableKind::ForwardIterator,
        Ok(IsIterable::BidirectionalIterator) => abi::IterableKind::BidirectionalIterator,
        Err(_) => abi::IterableKind::NotIterable,
    }
}

unsafe extern "C" fn object_make_iterator_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut abi::KValue,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let mut vm = KotoVm::from_api(api);
            let object = match borrow_object::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            match object.make_iterator(&mut vm) {
                Ok(value) => {
                    unsafe { *out = encode_value(api, value.into()) };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_iterator_next_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let mut vm = KotoVm::from_api(api);
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            match object.iterator_next(&mut vm) {
                Ok(Some(output)) => match KValue::try_from(output) {
                    Ok(value) => {
                        unsafe {
                            *out = encode_value(api, value);
                            *out_has_value = true;
                        }
                        abi::KotoStatus::ok()
                    }
                    Err(error) => error.into_status(),
                },
                Ok(None) => {
                    unsafe { *out_has_value = false };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

unsafe extern "C" fn object_iterator_next_back_trampoline<T>(
    host_api: *const abi::KotoHostApiV1,
    instance: abi::KObject,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus
where
    T: KotoObject,
{
    match catch_unwind(AssertUnwindSafe(|| {
        let api = unsafe { &*host_api };
        with_host_api(api, || {
            let mut vm = KotoVm::from_api(api);
            let mut object = match borrow_object_mut::<T>(api, instance) {
                Ok(object) => object,
                Err(error) => return error.into_status(),
            };
            match object.iterator_next_back(&mut vm) {
                Ok(Some(output)) => match KValue::try_from(output) {
                    Ok(value) => {
                        unsafe {
                            *out = encode_value(api, value);
                            *out_has_value = true;
                        }
                        abi::KotoStatus::ok()
                    }
                    Err(error) => error.into_status(),
                },
                Ok(None) => {
                    unsafe { *out_has_value = false };
                    abi::KotoStatus::ok()
                }
                Err(error) => error.into_status(),
            }
        })
    })) {
        Ok(status) => status,
        Err(_) => Error::new("plugin object callback panicked").into_status(),
    }
}

impl<T> From<T> for KObject
where
    T: KotoObject + 'static,
{
    fn from(value: T) -> Self {
        let api = current_host_api();
        let mut value = ManuallyDrop::new(value);
        let handle = unsafe {
            (api.object_make)(
                rust_object_v1::<T>(),
                abi::KotoObjectDataV1 {
                    struct_size: size_of::<abi::KotoObjectDataV1>(),
                    size: size_of::<T>().max(1),
                    align: std::mem::align_of::<T>(),
                    init: object_init_trampoline::<T>,
                    drop: object_drop_data_trampoline::<T>,
                    source: (&mut *value as *mut T).cast(),
                },
            )
        };

        Self {
            api: api as *const _,
            handle,
        }
    }
}
