mod object;
pub(crate) mod transfer;

use crate::{
    Borrow, BorrowMut, DisplayContext, IsIterable, KFunction, KIterator, KList, KMap,
    KNativeFunction, KObject, KRange, KString, KTuple, KValue, KotoObject, KotoVm, MetaKey, PtrMut,
    Result, ValueKey, error::Error,
};
use koto_api::{BinaryOp, KotoCollection, KotoType, ReadOp, UnaryOp, WriteOp};
use koto_ffi as abi;
use libloading::{Library, Symbol};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CString, c_void},
    hash::BuildHasherDefault,
    mem::{align_of, size_of},
    path::{Path, PathBuf},
    sync::Arc,
};

use self::{
    object::{PluginObjectData, make_plugin_object, plugin_object_v1},
    transfer::AbiTransfer,
};

thread_local! {
    static CURRENT_LIBRARY: RefCell<Option<Arc<LoadedLibrary>>> = const { RefCell::new(None) };
    static CURRENT_VM: Cell<*mut KotoVm> = const { Cell::new(std::ptr::null_mut()) };
}

/// A cache of loaded native modules.
pub(crate) type NativeModuleCache =
    HashMap<PathBuf, Option<LoadedNativeModule>, BuildHasherDefault<FxHasher>>;

pub(crate) struct LoadedNativeModule {
    pub exports: KMap,
    #[allow(dead_code)]
    pub library: Arc<LoadedLibrary>,
}

pub(crate) struct LoadedLibrary {
    library: Library,
}

const VALUE_VIEW_KEY_TAG: usize = 1;

unsafe fn take_abi<T: AbiTransfer>(abi: T::Abi) -> T {
    unsafe { T::from_abi(abi) }
}

unsafe fn clone_abi<T: AbiTransfer>(abi: T::Abi) -> T {
    unsafe { T::clone_from_abi(abi) }
}

impl AbiTransfer for KString {
    type Abi = abi::KString;

    unsafe fn into_abi(self) -> Self::Abi {
        KString::into_abi(self)
    }

    unsafe fn from_abi(abi: Self::Abi) -> Self {
        unsafe { KString::from_abi(abi) }
    }

    unsafe fn clone_from_abi(abi: Self::Abi) -> Self {
        unsafe { KString::clone_from_abi(abi) }
    }
}

fn value_view_from_value_ptr(value: *const KValue) -> abi::KValueView {
    abi::KValueView(value.cast())
}

fn value_view_from_key_ptr(key: *const ValueKey) -> abi::KValueView {
    abi::KValueView(((key as usize) | VALUE_VIEW_KEY_TAG) as *const c_void)
}

fn value_view_ptr(value: abi::KValueView) -> *const c_void {
    ((value.0 as usize) & !VALUE_VIEW_KEY_TAG) as *const c_void
}

fn value_view_is_key(value: abi::KValueView) -> bool {
    (value.0 as usize & VALUE_VIEW_KEY_TAG) != 0
}

fn opaque_fat_ptr_from_iterator(iterator: KIterator) -> abi::OpaqueHandle {
    unsafe { iterator.into_abi() }
}

fn iterator_from_opaque_fat_ptr(handle: abi::OpaqueHandle) -> KIterator {
    unsafe { KIterator::from_abi(handle) }
}

fn clone_iterator_from_opaque_fat_ptr(handle: abi::OpaqueHandle) -> KIterator {
    unsafe { KIterator::clone_from_abi(handle) }
}

fn opaque_fat_ptr_from_native_function(function: KNativeFunction) -> abi::OpaqueHandle {
    unsafe { function.into_abi() }
}

fn native_function_from_opaque_fat_ptr(handle: abi::OpaqueHandle) -> KNativeFunction {
    unsafe { KNativeFunction::from_abi(handle) }
}

fn clone_native_function_from_opaque_fat_ptr(handle: abi::OpaqueHandle) -> KNativeFunction {
    unsafe { KNativeFunction::clone_from_abi(handle) }
}

// The OS loader keeps the library alive behind this handle. The runtime pins an Arc alongside any
// plugin callbacks so the handle stays valid for the lifetime of exported functions and objects.
unsafe impl Send for LoadedLibrary {}
unsafe impl Sync for LoadedLibrary {}

struct AbiFunction {
    function: abi::KotoPluginFunction,
    user_data: *mut c_void,
    drop_user_data: abi::KotoPluginDrop,
    library: Arc<LoadedLibrary>,
}

unsafe impl Send for AbiFunction {}
unsafe impl Sync for AbiFunction {}

impl Drop for AbiFunction {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) };
    }
}

fn call_abi_function(wrapper: &AbiFunction, ctx: &mut crate::CallContext) -> Result<KValue> {
    let instance = match ctx.instance() {
        KValue::Null => abi::KValue::null(),
        instance => encode_abi_value(instance.clone()),
    };
    let args = ctx
        .args()
        .iter()
        .cloned()
        .map(encode_abi_value)
        .collect::<Vec<_>>();
    let call_ctx = abi::CallContext {
        instance,
        args: args.as_ptr(),
        arg_count: args.len(),
    };
    let mut out = abi::KValue::default();
    let status = with_current_library(wrapper.library.clone(), || {
        with_current_vm(ctx.vm, || unsafe {
            (wrapper.function)(&HOST_API, call_ctx, wrapper.user_data, &mut out)
        })
    });

    unsafe {
        value_free(instance);
        for arg in args {
            value_free(arg);
        }
    }

    if status.code == abi::KotoStatusCode::Ok {
        Ok(take_value(out))
    } else {
        Err(status_to_error(status))
    }
}

fn make_abi_native_function(wrapper: Arc<AbiFunction>) -> KNativeFunction {
    KNativeFunction::new(move |ctx| call_abi_function(&wrapper, ctx))
}

pub(crate) struct PluginObjectStorage {
    data: *mut c_void,
    layout: Layout,
    drop_data: abi::ObjectFnDropData,
}

unsafe impl Send for PluginObjectStorage {}
unsafe impl Sync for PluginObjectStorage {}

impl Drop for PluginObjectStorage {
    fn drop(&mut self) {
        unsafe {
            (self.drop_data)(self.data);
            std::alloc::dealloc(self.data as *mut u8, self.layout);
        }
    }
}

enum ObjectBorrowHandle {
    Shared(Borrow<'static, dyn KotoObject>),
    Mutable(BorrowMut<'static, dyn KotoObject>),
}

fn plugin_borrow_storage_size() -> usize {
    size_of::<[usize; abi::KOBJECT_BORROW_WORDS]>()
}

fn object_borrow_data_ptr(object: &dyn KotoObject) -> *mut c_void {
    if let Some(plugin_data) = (object as &dyn std::any::Any).downcast_ref::<PluginObjectData>() {
        plugin_data
            .storage
            .try_borrow()
            .map(|storage| storage.data)
            .unwrap_or(std::ptr::null_mut())
    } else {
        unsafe {
            let [data, _metadata]: [*const c_void; 2] =
                std::mem::transmute(object as *const dyn KotoObject);
            data as *mut c_void
        }
    }
}

fn make_object_borrow_token(token: ObjectBorrowHandle, data: *mut c_void) -> abi::KObjectBorrow {
    assert!(
        size_of::<ObjectBorrowHandle>() <= plugin_borrow_storage_size(),
        "plugin object borrow token storage is too small"
    );
    assert!(
        align_of::<ObjectBorrowHandle>() <= align_of::<abi::KObjectBorrow>(),
        "plugin object borrow token alignment is too large"
    );

    let mut result = abi::KObjectBorrow::default();
    result.data = data;
    unsafe {
        std::ptr::write(
            result.storage.as_mut_ptr().cast::<ObjectBorrowHandle>(),
            token,
        );
    }
    result
}

fn make_object_borrow_mut_token(
    token: ObjectBorrowHandle,
    data: *mut c_void,
) -> abi::KObjectBorrowMut {
    assert!(
        size_of::<ObjectBorrowHandle>() <= plugin_borrow_storage_size(),
        "plugin object borrow token storage is too small"
    );
    assert!(
        align_of::<ObjectBorrowHandle>() <= align_of::<abi::KObjectBorrowMut>(),
        "plugin object borrow token alignment is too large"
    );

    let mut result = abi::KObjectBorrowMut::default();
    result.data = data;
    unsafe {
        std::ptr::write(
            result.storage.as_mut_ptr().cast::<ObjectBorrowHandle>(),
            token,
        );
    }
    result
}

fn borrowed_object<'a>(borrow: &'a abi::KObjectBorrow) -> Result<&'a dyn KotoObject> {
    if !borrow.is_valid() {
        return Err(Error::from("null object borrow"));
    }

    match unsafe { &*borrow.storage.as_ptr().cast::<ObjectBorrowHandle>() } {
        ObjectBorrowHandle::Shared(object) => Ok(&**object),
        ObjectBorrowHandle::Mutable(object) => Ok(&**object),
    }
}

fn borrowed_object_mut<'a>(
    borrow: &'a mut abi::KObjectBorrowMut,
) -> Result<&'a mut dyn KotoObject> {
    if !borrow.is_valid() {
        return Err(Error::from("null object borrow"));
    }

    match unsafe { &mut *borrow.storage.as_mut_ptr().cast::<ObjectBorrowHandle>() } {
        ObjectBorrowHandle::Mutable(object) => Ok(&mut **object),
        ObjectBorrowHandle::Shared(_) => Err(Error::from("object borrow is immutable")),
    }
}

pub(crate) fn with_current_library<T>(library: Arc<LoadedLibrary>, f: impl FnOnce() -> T) -> T {
    CURRENT_LIBRARY.with(|slot| {
        let previous = slot.replace(Some(library));
        let result = f();
        *slot.borrow_mut() = previous;
        result
    })
}

pub(crate) fn with_current_vm<T>(vm: &mut KotoVm, f: impl FnOnce() -> T) -> T {
    CURRENT_VM.with(|slot| {
        let previous = slot.replace(vm as *mut _);
        let result = f();
        slot.set(previous);
        result
    })
}

fn current_library() -> Arc<LoadedLibrary> {
    CURRENT_LIBRARY
        .with_borrow(|library| library.clone())
        .expect("plugin operation called without an active library")
}

fn current_vm() -> Result<&'static mut KotoVm> {
    CURRENT_VM.with(|slot| {
        let vm = slot.get();
        if vm.is_null() {
            Err(Error::from(
                "plugin VM operation called without an active runtime VM",
            ))
        } else {
            Ok(unsafe { &mut *vm })
        }
    })
}

pub(crate) fn abi_string_slice(s: &str) -> abi::KStringSlice {
    abi::KStringSlice {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

pub(crate) fn string_slice_to_string(slice: abi::KStringSlice) -> String {
    if slice.ptr.is_null() || slice.len == 0 {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }
}

pub(crate) fn status_to_error(status: abi::KotoStatus) -> Error {
    if !status.error.is_null() {
        return unsafe { *Box::from_raw(status.error.cast::<Error>()) };
    }

    if status.message.is_null() {
        return Error::from("dynamic module operation failed");
    }

    let message = unsafe { CString::from_raw(status.message) };
    Error::from(message.to_string_lossy().into_owned())
}

unsafe extern "C" fn clone_runtime_error(error: *mut c_void) -> *mut c_void {
    let error = unsafe { &*(error.cast::<Error>()) };
    Box::into_raw(Box::new(error.clone())).cast()
}

unsafe extern "C" fn free_runtime_error(error: *mut c_void) {
    if !error.is_null() {
        unsafe { drop(Box::from_raw(error.cast::<Error>())) };
    }
}

pub(crate) fn encode_abi_value(value: KValue) -> abi::KValue {
    match value {
        KValue::Null => abi::KValue::null(),
        KValue::Bool(value) => abi::KValue {
            kind: abi::KValueKind::Bool,
            data: abi::KValueData { bool_value: value },
        },
        KValue::Number(value) if value.is_i64() => abi::KValue {
            kind: abi::KValueKind::I64,
            data: abi::KValueData {
                i64_value: value.into(),
            },
        },
        KValue::Number(value) => abi::KValue {
            kind: abi::KValueKind::F64,
            data: abi::KValueData {
                f64_value: value.into(),
            },
        },
        KValue::Range(value) => abi::KValue {
            kind: abi::KValueKind::Range,
            data: abi::KValueData {
                range_value: abi::KotoRange {
                    start: value.start().unwrap_or_default(),
                    has_start: value.start().is_some(),
                    end: value.end().map_or(0, |(end, _)| end),
                    has_end: value.end().is_some(),
                    end_inclusive: value.end().is_some_and(|(_, inclusive)| inclusive),
                },
            },
        },
        KValue::Str(value) => abi::KValue {
            kind: abi::KValueKind::String,
            data: abi::KValueData {
                string_value: value.into_abi(),
            },
        },
        KValue::List(value) => abi::KValue {
            kind: abi::KValueKind::List,
            data: abi::KValueData {
                handle: unsafe { value.into_abi().0 },
            },
        },
        KValue::Tuple(value) => abi::KValue {
            kind: abi::KValueKind::Tuple,
            data: abi::KValueData {
                tuple_value: unsafe { value.into_abi() },
            },
        },
        KValue::Map(value) => abi::KValue {
            kind: abi::KValueKind::Map,
            data: abi::KValueData {
                map_value: unsafe { value.into_abi() },
            },
        },
        KValue::Function(value) => abi::KValue {
            kind: abi::KValueKind::Function,
            data: abi::KValueData {
                function_value: unsafe { value.into_abi() },
            },
        },
        KValue::NativeFunction(value) => abi::KValue {
            kind: abi::KValueKind::NativeFunction,
            data: abi::KValueData {
                native_function_value: opaque_fat_ptr_from_native_function(value),
            },
        },
        KValue::Iterator(value) => abi::KValue {
            kind: abi::KValueKind::Iterator,
            data: abi::KValueData {
                iterator_value: opaque_fat_ptr_from_iterator(value),
            },
        },
        KValue::Object(value) => abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: unsafe { value.into_abi() },
            },
        },
        value => panic!(
            "unsupported runtime value for plugin ABI v1: {}",
            value.type_as_string()
        ),
    }
}

fn range_from_abi(value: abi::KotoRange) -> KRange {
    let start = value.has_start.then_some(value.start);
    let end = value.has_end.then_some((value.end, value.end_inclusive));
    KRange::new(start, end)
}

fn list_handle(value: abi::KValue) -> Option<*const crate::KCell<crate::ValueVec>> {
    matches!(value.kind, abi::KValueKind::List)
        .then(|| unsafe { value.data.handle as *const crate::KCell<crate::ValueVec> })
        .filter(|handle| !handle.is_null())
}

fn list_handle_from_abi(list: abi::KList) -> Option<*const crate::KCell<crate::ValueVec>> {
    (!list.0.is_null()).then_some(list.0 as *const crate::KCell<crate::ValueVec>)
}

fn abi_list(list: KList) -> abi::KList {
    unsafe { list.into_abi() }
}

fn clone_value_handle(value: abi::KValue) -> KValue {
    match value.kind {
        abi::KValueKind::Null => KValue::Null,
        abi::KValueKind::Bool => KValue::Bool(unsafe { value.data.bool_value }),
        abi::KValueKind::I64 => KValue::Number(unsafe { value.data.i64_value }.into()),
        abi::KValueKind::F64 => KValue::Number(unsafe { value.data.f64_value }.into()),
        abi::KValueKind::Range => KValue::Range(range_from_abi(unsafe { value.data.range_value })),
        abi::KValueKind::String => KValue::Str(unsafe { clone_abi(value.data.string_value) }),
        abi::KValueKind::List => match list_handle(value) {
            Some(handle) => KValue::List(unsafe { clone_abi(abi::KList(handle as *mut c_void)) }),
            _ => KValue::Null,
        },
        abi::KValueKind::Tuple => KValue::Tuple(unsafe { clone_abi(value.data.tuple_value) }),
        abi::KValueKind::Map => KValue::Map(unsafe { clone_abi(value.data.map_value) }),
        abi::KValueKind::Function => {
            KValue::Function(unsafe { clone_abi(value.data.function_value) })
        }
        abi::KValueKind::NativeFunction => {
            KValue::NativeFunction(clone_native_function_from_opaque_fat_ptr(unsafe {
                value.data.native_function_value
            }))
        }
        abi::KValueKind::Iterator => KValue::Iterator(clone_iterator_from_opaque_fat_ptr(unsafe {
            value.data.iterator_value
        })),
        abi::KValueKind::Object => {
            let handle = unsafe { value.data.object_value };
            if handle.data.is_null() {
                KValue::Null
            } else {
                KValue::Object(unsafe { clone_abi(handle) })
            }
        }
        abi::KValueKind::Unsupported => {
            panic!("attempted to clone unsupported runtime value through plugin ABI")
        }
    }
}

fn clone_args_from_handles(args: *const abi::KValue, arg_count: usize) -> Vec<KValue> {
    let args = unsafe { std::slice::from_raw_parts(args, arg_count) };
    args.iter().copied().map(clone_value_handle).collect()
}

pub(crate) fn take_value(value: abi::KValue) -> KValue {
    match value.kind {
        abi::KValueKind::Null => KValue::Null,
        abi::KValueKind::Bool => KValue::Bool(unsafe { value.data.bool_value }),
        abi::KValueKind::I64 => KValue::Number(unsafe { value.data.i64_value }.into()),
        abi::KValueKind::F64 => KValue::Number(unsafe { value.data.f64_value }.into()),
        abi::KValueKind::Range => KValue::Range(range_from_abi(unsafe { value.data.range_value })),
        abi::KValueKind::Function => {
            KValue::Function(unsafe { take_abi(value.data.function_value) })
        }
        abi::KValueKind::NativeFunction => {
            KValue::NativeFunction(native_function_from_opaque_fat_ptr(unsafe {
                value.data.native_function_value
            }))
        }
        abi::KValueKind::Iterator => KValue::Iterator(iterator_from_opaque_fat_ptr(unsafe {
            value.data.iterator_value
        })),
        abi::KValueKind::Unsupported => {
            panic!("attempted to take unsupported runtime value through plugin ABI")
        }
        abi::KValueKind::Object => {
            let handle = unsafe { value.data.object_value };
            assert!(
                !handle.data.is_null(),
                "plugin returned a null object handle"
            );
            KValue::Object(unsafe { take_abi(handle) })
        }
        abi::KValueKind::Map => KValue::Map(unsafe { take_abi(value.data.map_value) }),
        abi::KValueKind::String => KValue::Str(unsafe { take_abi(value.data.string_value) }),
        abi::KValueKind::Tuple => KValue::Tuple(unsafe { take_abi(value.data.tuple_value) }),
        abi::KValueKind::List => {
            let list = abi::KList(unsafe { value.data.handle });
            assert!(!list.0.is_null(), "plugin returned a null list handle");
            KValue::List(unsafe { take_abi(list) })
        }
    }
}

fn error_to_status(error: Error) -> abi::KotoStatus {
    let is_unimplemented = error.is_unimplemented_error();
    abi::KotoStatus {
        code: abi::KotoStatusCode::Error,
        error: Box::into_raw(Box::new(error)).cast(),
        clone_error: clone_runtime_error as *const c_void,
        free_error: free_runtime_error as *const c_void,
        is_unimplemented,
        message: std::ptr::null_mut(),
    }
}

fn unary_op_from_abi(op: abi::UnaryOp) -> UnaryOp {
    match op {
        abi::UnaryOp::Debug => UnaryOp::Debug,
        abi::UnaryOp::Display => UnaryOp::Display,
        abi::UnaryOp::Negate => UnaryOp::Negate,
        abi::UnaryOp::Iterator => UnaryOp::Iterator,
        abi::UnaryOp::Next => UnaryOp::Next,
        abi::UnaryOp::NextBack => UnaryOp::NextBack,
        abi::UnaryOp::Size => UnaryOp::Size,
    }
}

fn binary_op_from_abi(op: abi::BinaryOp) -> BinaryOp {
    match op {
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
        abi::BinaryOp::AddAssign => BinaryOp::AddAssign,
        abi::BinaryOp::SubtractAssign => BinaryOp::SubtractAssign,
        abi::BinaryOp::MultiplyAssign => BinaryOp::MultiplyAssign,
        abi::BinaryOp::DivideAssign => BinaryOp::DivideAssign,
        abi::BinaryOp::RemainderAssign => BinaryOp::RemainderAssign,
        abi::BinaryOp::PowerAssign => BinaryOp::PowerAssign,
        abi::BinaryOp::Less => BinaryOp::Less,
        abi::BinaryOp::LessOrEqual => BinaryOp::LessOrEqual,
        abi::BinaryOp::Greater => BinaryOp::Greater,
        abi::BinaryOp::GreaterOrEqual => BinaryOp::GreaterOrEqual,
        abi::BinaryOp::Equal => BinaryOp::Equal,
        abi::BinaryOp::NotEqual => BinaryOp::NotEqual,
    }
}

fn read_op_from_abi(op: abi::ReadOp) -> ReadOp {
    match op {
        abi::ReadOp::Index => ReadOp::Index,
        abi::ReadOp::Access => ReadOp::Access,
    }
}

fn write_op_from_abi(op: abi::WriteOp) -> WriteOp {
    match op {
        abi::WriteOp::IndexAssign => WriteOp::IndexAssign,
        abi::WriteOp::AccessAssign => WriteOp::AccessAssign,
    }
}

fn meta_key_from_abi(key: abi::MetaKey) -> MetaKey {
    match key.kind {
        abi::MetaKeyKind::UnaryOp => {
            MetaKey::UnaryOp(unary_op_from_abi(unsafe { key.data.unary_op }))
        }
        abi::MetaKeyKind::BinaryOp => {
            MetaKey::BinaryOp(binary_op_from_abi(unsafe { key.data.binary_op }))
        }
        abi::MetaKeyKind::ReadOp => MetaKey::ReadOp(read_op_from_abi(unsafe { key.data.read_op })),
        abi::MetaKeyKind::WriteOp => {
            MetaKey::WriteOp(write_op_from_abi(unsafe { key.data.write_op }))
        }
        abi::MetaKeyKind::Call => MetaKey::Call,
        abi::MetaKeyKind::Named => {
            MetaKey::Named(string_slice_to_string(unsafe { key.data.string }).into())
        }
        abi::MetaKeyKind::Test => {
            MetaKey::Test(string_slice_to_string(unsafe { key.data.string }).into())
        }
        abi::MetaKeyKind::PreTest => MetaKey::PreTest,
        abi::MetaKeyKind::PostTest => MetaKey::PostTest,
        abi::MetaKeyKind::Main => MetaKey::Main,
        abi::MetaKeyKind::Type => MetaKey::Type,
        abi::MetaKeyKind::Base => MetaKey::Base,
    }
}

unsafe extern "C" fn map_new_with_type(type_name: abi::KStringSlice) -> abi::KMap {
    let type_name = string_slice_to_string(type_name);
    unsafe { KMap::with_type(&type_name).into_abi() }
}

unsafe extern "C" fn map_insert_value(map: abi::KMap, key: abi::KStringSlice, value: abi::KValue) {
    let map = unsafe { clone_abi::<KMap>(map) };
    let key = string_slice_to_string(key);
    map.insert(key.as_str(), take_value(value));
}

unsafe extern "C" fn map_insert_meta_value(map: abi::KMap, key: abi::MetaKey, value: abi::KValue) {
    let mut map = unsafe { clone_abi::<KMap>(map) };
    map.insert_meta(meta_key_from_abi(key), take_value(value));
}

unsafe extern "C" fn native_function_make(
    function: abi::KotoPluginFunction,
    user_data: *mut c_void,
    drop_user_data: abi::KotoPluginDrop,
) -> abi::OpaqueHandle {
    let wrapper = Arc::new(AbiFunction {
        function,
        user_data,
        drop_user_data,
        library: current_library(),
    });

    opaque_fat_ptr_from_native_function(make_abi_native_function(wrapper))
}

unsafe extern "C" fn value_make_null() -> abi::KValue {
    abi::KValue::null()
}

unsafe extern "C" fn value_make_bool(value: bool) -> abi::KValue {
    abi::KValue {
        kind: abi::KValueKind::Bool,
        data: abi::KValueData { bool_value: value },
    }
}

unsafe extern "C" fn value_make_i64(value: i64) -> abi::KValue {
    abi::KValue {
        kind: abi::KValueKind::I64,
        data: abi::KValueData { i64_value: value },
    }
}

unsafe extern "C" fn value_make_f64(value: f64) -> abi::KValue {
    abi::KValue {
        kind: abi::KValueKind::F64,
        data: abi::KValueData { f64_value: value },
    }
}

unsafe extern "C" fn value_make_range(value: abi::KotoRange) -> abi::KValue {
    abi::KValue {
        kind: abi::KValueKind::Range,
        data: abi::KValueData { range_value: value },
    }
}

unsafe extern "C" fn string_make(value: abi::KStringSlice) -> abi::KString {
    KString::from(string_slice_to_string(value)).into_abi()
}

unsafe extern "C" fn tuple_make(values: *const abi::KValue, len: usize) -> abi::KTuple {
    let values = unsafe { std::slice::from_raw_parts(values, len) };
    unsafe { KTuple::from(values.iter().copied().map(take_value).collect::<Vec<_>>()).into_abi() }
}

unsafe extern "C" fn list_make(values: *const abi::KValue, len: usize) -> abi::KList {
    let values = unsafe { std::slice::from_raw_parts(values, len) };
    let values = values.iter().copied().map(take_value).collect::<Vec<_>>();
    abi_list(KList::from_slice(&values))
}

unsafe extern "C" fn map_make(entries: *const abi::KotoMapEntry, len: usize) -> abi::KMap {
    let entries = unsafe { std::slice::from_raw_parts(entries, len) };
    let data = entries
        .iter()
        .filter_map(|entry| {
            let key = take_value(entry.key);
            let value = take_value(entry.value);
            ValueKey::try_from(key).ok().map(|key| (key, value))
        })
        .collect();

    unsafe { KMap::with_data(data).into_abi() }
}

unsafe extern "C" fn object_make(
    object_v1: *const abi::KotoPluginObjectV1,
    object_data: abi::KotoObjectDataV1,
) -> abi::KObject {
    assert!(!object_v1.is_null(), "plugin object descriptor was null");
    assert!(object_data.size > 0, "plugin object size was zero");
    assert!(object_data.align > 0, "plugin object alignment was zero");

    let layout = Layout::from_size_align(object_data.size, object_data.align)
        .expect("invalid plugin object layout");
    let data = unsafe { std::alloc::alloc(layout) as *mut c_void };
    assert!(
        !data.is_null(),
        "runtime failed to allocate plugin object storage"
    );

    unsafe { (object_data.init)(data, object_data.source) };

    let storage = PtrMut::from(PluginObjectStorage {
        data,
        layout,
        drop_data: object_data.drop,
    });

    unsafe { make_plugin_object(object_v1, storage, current_library()).into_abi() }
}

unsafe extern "C" fn value_clone(value: abi::KValue) -> abi::KValue {
    encode_abi_value(clone_value_handle(value))
}

unsafe extern "C" fn value_view_clone(value: abi::KValueView) -> abi::KValue {
    let ptr = value_view_ptr(value);
    if ptr.is_null() {
        abi::KValue::default()
    } else if value_view_is_key(value) {
        let key = unsafe { &*(ptr as *const ValueKey) };
        encode_abi_value(key.value().clone())
    } else {
        let value = unsafe { &*(ptr as *const KValue) };
        encode_abi_value(value.clone())
    }
}

pub(crate) unsafe extern "C" fn value_free(value: abi::KValue) {
    match value.kind {
        abi::KValueKind::Function => {
            let _ = unsafe { take_abi::<KFunction>(value.data.function_value) };
        }
        abi::KValueKind::NativeFunction => {
            let _ =
                native_function_from_opaque_fat_ptr(unsafe { value.data.native_function_value });
        }
        abi::KValueKind::Iterator => {
            let _ = iterator_from_opaque_fat_ptr(unsafe { value.data.iterator_value });
        }
        abi::KValueKind::String => {
            let _ = unsafe { take_abi::<KString>(value.data.string_value) };
        }
        abi::KValueKind::Map => {
            let _ = unsafe { take_abi::<KMap>(value.data.map_value) };
        }
        abi::KValueKind::Object => {
            let handle = unsafe { value.data.object_value };
            if !handle.data.is_null() {
                let _ = unsafe { take_abi::<KObject>(handle) };
            }
        }
        abi::KValueKind::List => {
            if let Some(handle) = list_handle(value) {
                let _ = unsafe { take_abi::<KList>(abi::KList(handle as *mut c_void)) };
            }
        }
        abi::KValueKind::Tuple => {
            let _ = unsafe { take_abi::<KTuple>(value.data.tuple_value) };
        }
        abi::KValueKind::Unsupported => {
            panic!("attempted to free unsupported runtime value through plugin ABI")
        }
        _ => {}
    }
}

unsafe extern "C" fn value_is_same_instance(a: abi::KValue, b: abi::KValue) -> bool {
    clone_value_handle(a).is_same_instance(&clone_value_handle(b))
}

unsafe extern "C" fn value_kind(value: abi::KValue) -> abi::KValueKind {
    value.kind
}

unsafe extern "C" fn value_as_bool(value: abi::KValue) -> bool {
    matches!(value.kind, abi::KValueKind::Bool)
        .then(|| unsafe { value.data.bool_value })
        .unwrap_or(false)
}

unsafe extern "C" fn value_as_i64(value: abi::KValue) -> i64 {
    matches!(value.kind, abi::KValueKind::I64)
        .then(|| unsafe { value.data.i64_value })
        .unwrap_or_default()
}

unsafe extern "C" fn value_as_f64(value: abi::KValue) -> f64 {
    matches!(value.kind, abi::KValueKind::F64)
        .then(|| unsafe { value.data.f64_value })
        .unwrap_or_default()
}

unsafe extern "C" fn value_as_range(value: abi::KValue) -> abi::KotoRange {
    matches!(value.kind, abi::KValueKind::Range)
        .then(|| unsafe { value.data.range_value })
        .unwrap_or_default()
}

unsafe extern "C" fn string_as_slice(string: abi::KString) -> abi::KStringSlice {
    let string = unsafe { clone_abi::<KString>(string) };
    abi::KStringSlice {
        ptr: string.as_bytes().as_ptr(),
        len: string.len(),
    }
}

unsafe extern "C" fn tuple_len(tuple: abi::KTuple) -> usize {
    unsafe { clone_abi::<KTuple>(tuple) }.len()
}

unsafe extern "C" fn tuple_data(tuple: abi::KTuple) -> abi::KValueSlice {
    let tuple = unsafe { clone_abi::<KTuple>(tuple) };
    let data = tuple.data();
    abi::KValueSlice {
        data: data.as_ptr().cast::<c_void>(),
        len: data.len(),
        stride: size_of::<KValue>(),
    }
}

unsafe extern "C" fn tuple_get(tuple: abi::KTuple, index: usize) -> abi::KValue {
    unsafe { clone_abi::<KTuple>(tuple) }
        .get(index)
        .cloned()
        .map(encode_abi_value)
        .unwrap_or_default()
}

unsafe extern "C" fn list_len(list: abi::KList) -> usize {
    match list_handle_from_abi(list) {
        Some(handle) => unsafe { clone_abi::<KList>(abi::KList(handle as *mut c_void)) }.len(),
        _ => 0,
    }
}

unsafe extern "C" fn list_data(list: abi::KList) -> abi::KValueSlice {
    match list_handle_from_abi(list) {
        Some(handle) => {
            let list = unsafe { clone_abi::<KList>(abi::KList(handle as *mut c_void)) };
            let data = list.data();
            abi::KValueSlice {
                data: data.as_ptr().cast::<c_void>(),
                len: data.len(),
                stride: size_of::<KValue>(),
            }
        }
        _ => abi::KValueSlice::default(),
    }
}

unsafe extern "C" fn list_get(list: abi::KList, index: usize) -> abi::KValue {
    match list_handle_from_abi(list) {
        Some(handle) => unsafe { clone_abi::<KList>(abi::KList(handle as *mut c_void)) }
            .data()
            .get(index)
            .cloned()
            .map(encode_abi_value)
            .unwrap_or_default(),
        _ => abi::KValue::default(),
    }
}

unsafe extern "C" fn list_set(
    list: abi::KList,
    index: usize,
    item: abi::KValue,
) -> abi::KotoStatus {
    let item = take_value(item);

    match list_handle_from_abi(list) {
        Some(handle) => {
            let result = unsafe { clone_abi::<KList>(abi::KList(handle as *mut c_void)) };
            let mut data = result.data_mut();
            if let Some(slot) = data.get_mut(index) {
                *slot = item;
                abi::KotoStatus::ok()
            } else {
                error_to_status(Error::from(format!("invalid list index ({index})")))
            }
        }
        _ => error_to_status(Error::from("expected a List value")),
    }
}

unsafe extern "C" fn map_len(map: abi::KMap) -> usize {
    unsafe { clone_abi::<KMap>(map) }.len()
}

unsafe extern "C" fn map_data(map: abi::KMap) -> abi::KMapData {
    abi::KMapData {
        data: map.data,
        len: unsafe { clone_abi::<KMap>(map) }.len(),
    }
}

unsafe extern "C" fn map_data_get_entry(map: abi::KMapData, index: usize) -> abi::KMapEntryView {
    if map.data.is_null() {
        return abi::KMapEntryView::default();
    }

    let data = unsafe { &*(map.data as *const crate::KCell<crate::ValueMap>) }.borrow();
    data.get_index(index)
        .map_or_else(abi::KMapEntryView::default, |(key, value)| {
            abi::KMapEntryView {
                key: value_view_from_key_ptr(key as *const ValueKey),
                value: value_view_from_value_ptr(value as *const KValue),
            }
        })
}

unsafe extern "C" fn map_key_at(map: abi::KMap, index: usize) -> abi::KValue {
    unsafe { clone_abi::<KMap>(map) }
        .data()
        .get_index(index)
        .map(|(key, _)| encode_abi_value(key.value().clone()))
        .unwrap_or_default()
}

unsafe extern "C" fn map_value_at(map: abi::KMap, index: usize) -> abi::KValue {
    unsafe { clone_abi::<KMap>(map) }
        .data()
        .get_index(index)
        .map(|(_, value)| encode_abi_value(value.clone()))
        .unwrap_or_default()
}

unsafe extern "C" fn map_swap_indices(map: abi::KMap, a: usize, b: usize) -> abi::KotoStatus {
    let result = unsafe { clone_abi::<KMap>(map) };
    let len = result.len();
    if a >= len || b >= len {
        error_to_status(Error::from(format!("invalid map indices ({a}, {b})")))
    } else {
        result.data_mut().swap_indices(a, b);
        abi::KotoStatus::ok()
    }
}

unsafe extern "C" fn map_contains_meta_read(map: abi::KMap, op: abi::ReadOp) -> bool {
    unsafe { clone_abi::<KMap>(map) }.contains_meta_key(&read_op_from_abi(op).into())
}

unsafe extern "C" fn map_get_meta_read(map: abi::KMap, op: abi::ReadOp) -> abi::KValue {
    unsafe { clone_abi::<KMap>(map) }
        .get_meta_value(&read_op_from_abi(op).into())
        .map(encode_abi_value)
        .unwrap_or_default()
}

unsafe extern "C" fn map_contains_meta_write(map: abi::KMap, op: abi::WriteOp) -> bool {
    unsafe { clone_abi::<KMap>(map) }.contains_meta_key(&write_op_from_abi(op).into())
}

unsafe extern "C" fn map_get_meta_write(map: abi::KMap, op: abi::WriteOp) -> abi::KValue {
    unsafe { clone_abi::<KMap>(map) }
        .get_meta_value(&write_op_from_abi(op).into())
        .map(encode_abi_value)
        .unwrap_or_default()
}

unsafe extern "C" fn object_v1(object: abi::KObject) -> *const abi::KotoPluginObjectV1 {
    if object.data.is_null() {
        return std::ptr::null();
    }

    plugin_object_v1(&unsafe { clone_abi::<KObject>(object) }).unwrap_or(std::ptr::null())
}

fn clone_object_handle(object: abi::KObject) -> Result<KObject> {
    if object.data.is_null() {
        Err(Error::from("null object handle"))
    } else {
        Ok(unsafe { clone_abi(object) })
    }
}

unsafe extern "C" fn object_borrow(object: abi::KObject) -> abi::KObjectBorrow {
    let Ok(object) = clone_object_handle(object) else {
        return abi::KObjectBorrow::default();
    };

    match object.try_borrow() {
        Ok(borrow) => {
            let borrow: Borrow<'static, dyn KotoObject> = unsafe { std::mem::transmute(borrow) };
            let data = object_borrow_data_ptr(&*borrow);
            make_object_borrow_token(ObjectBorrowHandle::Shared(borrow), data)
        }
        Err(_) => abi::KObjectBorrow::default(),
    }
}

unsafe extern "C" fn object_borrow_mut(object: abi::KObject) -> abi::KObjectBorrowMut {
    let Ok(object) = clone_object_handle(object) else {
        return abi::KObjectBorrowMut::default();
    };

    match object.try_borrow_mut() {
        Ok(borrow) => {
            let borrow: BorrowMut<'static, dyn KotoObject> = unsafe { std::mem::transmute(borrow) };
            let data = object_borrow_data_ptr(&*borrow);
            make_object_borrow_mut_token(ObjectBorrowHandle::Mutable(borrow), data)
        }
        Err(_) => abi::KObjectBorrowMut::default(),
    }
}

unsafe extern "C" fn object_borrow_free(borrow: abi::KObjectBorrow) {
    if borrow.is_valid() {
        unsafe {
            std::ptr::drop_in_place(
                borrow.storage.as_ptr().cast::<ObjectBorrowHandle>() as *mut ObjectBorrowHandle
            );
        }
    }
}

unsafe extern "C" fn object_borrow_mut_free(borrow: abi::KObjectBorrowMut) {
    if borrow.is_valid() {
        unsafe {
            std::ptr::drop_in_place(
                borrow.storage.as_ptr().cast::<ObjectBorrowHandle>() as *mut ObjectBorrowHandle
            );
        }
    }
}

unsafe extern "C" fn object_borrow_type_string(borrow: abi::KObjectBorrow) -> abi::KString {
    borrowed_object(&borrow)
        .map(|object| KotoType::type_string(&*object).into_abi())
        .unwrap_or_default()
}

unsafe extern "C" fn object_borrow_named_value(
    borrow: abi::KObjectBorrow,
    key: abi::KStringSlice,
    out: *mut abi::KValue,
    out_found: *mut bool,
) -> abi::KotoStatus {
    let key = string_slice_to_string(key);

    match borrowed_object(&borrow).and_then(|object| object.access(&key.into())) {
        Ok(Some(value)) => {
            unsafe {
                *out = encode_abi_value(value);
                *out_found = true;
            }
            abi::KotoStatus::ok()
        }
        Ok(None) => {
            unsafe { *out_found = false };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_named_value_assign(
    mut borrow: abi::KObjectBorrowMut,
    key: abi::KStringSlice,
    value: abi::KValue,
) -> abi::KotoStatus {
    let key = string_slice_to_string(key);
    let value = take_value(value);

    match borrowed_object_mut(&mut borrow)
        .and_then(|object| object.access_assign(&key.into(), &value))
    {
        Ok(()) => abi::KotoStatus::ok(),
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_iterable_kind(borrow: abi::KObjectBorrow) -> abi::IterableKind {
    match borrowed_object(&borrow).and_then(|object| object.is_iterable()) {
        Ok(IsIterable::NotIterable) => abi::IterableKind::NotIterable,
        Ok(IsIterable::Iterable) => abi::IterableKind::Iterable,
        Ok(IsIterable::ForwardIterator) => abi::IterableKind::ForwardIterator,
        Ok(IsIterable::BidirectionalIterator) => abi::IterableKind::BidirectionalIterator,
        Err(_) => abi::IterableKind::NotIterable,
    }
}

unsafe extern "C" fn object_borrow_iterator_next(
    mut borrow: abi::KObjectBorrowMut,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match borrowed_object_mut(&mut borrow).and_then(|object| object.iterator_next(vm)) {
        Ok(Some(output)) => match KValue::try_from(output) {
            Ok(value) => {
                unsafe {
                    *out = encode_abi_value(value);
                    *out_has_value = true;
                }
                abi::KotoStatus::ok()
            }
            Err(error) => error_to_status(error),
        },
        Ok(None) => {
            unsafe { *out_has_value = false };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_iterator_next_back(
    mut borrow: abi::KObjectBorrowMut,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match borrowed_object_mut(&mut borrow).and_then(|object| object.iterator_next_back(vm)) {
        Ok(Some(output)) => match KValue::try_from(output) {
            Ok(value) => {
                unsafe {
                    *out = encode_abi_value(value);
                    *out_has_value = true;
                }
                abi::KotoStatus::ok()
            }
            Err(error) => error_to_status(error),
        },
        Ok(None) => {
            unsafe { *out_has_value = false };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_display(borrow: abi::KObjectBorrow) -> abi::KString {
    borrowed_object(&borrow)
        .and_then(|object| {
            let mut ctx = DisplayContext::default();
            object.display(&mut ctx)?;
            Ok(KString::from(ctx.result()).into_abi())
        })
        .unwrap_or_default()
}

unsafe extern "C" fn object_borrow_size(
    borrow: abi::KObjectBorrow,
    out: *mut usize,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    match borrowed_object(&borrow).and_then(|object| object.size()) {
        Ok(Some(size)) => {
            unsafe {
                *out = size;
                *out_has_value = true;
            }
            abi::KotoStatus::ok()
        }
        Ok(None) => {
            unsafe { *out_has_value = false };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_index(
    borrow: abi::KObjectBorrow,
    index: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let index = take_value(index);

    match borrowed_object(&borrow).and_then(|object| object.index(&index)) {
        Ok(value) => {
            unsafe { *out = encode_abi_value(value) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_index_assign(
    mut borrow: abi::KObjectBorrowMut,
    index: abi::KValue,
    value: abi::KValue,
) -> abi::KotoStatus {
    let index = take_value(index);
    let value = take_value(value);

    match borrowed_object_mut(&mut borrow).and_then(|object| object.index_assign(&index, &value)) {
        Ok(()) => abi::KotoStatus::ok(),
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_is_callable(
    borrow: abi::KObjectBorrow,
    out: *mut bool,
) -> abi::KotoStatus {
    match borrowed_object(&borrow).and_then(|object| object.is_callable()) {
        Ok(value) => {
            unsafe { *out = value };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_call(
    mut borrow: abi::KObjectBorrowMut,
    ctx: abi::CallContext,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let instance = take_value(ctx.instance);
    let args = unsafe { std::slice::from_raw_parts(ctx.args, ctx.arg_count) }
        .iter()
        .copied()
        .map(take_value)
        .collect::<Vec<_>>();
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };
    match borrowed_object_mut(&mut borrow)
        .and_then(|object| vm.call_borrowed_object(object, instance, &args))
    {
        Ok(value) => {
            unsafe { *out = encode_abi_value(value) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_unary_op(
    borrow: abi::KObjectBorrow,
    op: abi::UnaryOp,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let result = match op {
        abi::UnaryOp::Negate => borrowed_object(&borrow).and_then(|object| object.negate()),
        _ => Err(Error::from(format!("unsupported object unary op: {op:?}"))),
    };

    match result {
        Ok(value) => {
            unsafe { *out = encode_abi_value(value) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_binary_op(
    borrow: abi::KObjectBorrow,
    op: abi::BinaryOp,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let rhs = take_value(rhs);

    let result = borrowed_object(&borrow).and_then(|object| match op {
        abi::BinaryOp::Add => object.add(&rhs),
        abi::BinaryOp::AddRhs => object.add_rhs(&rhs),
        abi::BinaryOp::Subtract => object.subtract(&rhs),
        abi::BinaryOp::SubtractRhs => object.subtract_rhs(&rhs),
        abi::BinaryOp::Multiply => object.multiply(&rhs),
        abi::BinaryOp::MultiplyRhs => object.multiply_rhs(&rhs),
        abi::BinaryOp::Divide => object.divide(&rhs),
        abi::BinaryOp::DivideRhs => object.divide_rhs(&rhs),
        abi::BinaryOp::Remainder => object.remainder(&rhs),
        abi::BinaryOp::RemainderRhs => object.remainder_rhs(&rhs),
        abi::BinaryOp::Power => object.power(&rhs),
        abi::BinaryOp::PowerRhs => object.power_rhs(&rhs),
        abi::BinaryOp::Less => object.less(&rhs).map(Into::into),
        abi::BinaryOp::LessOrEqual => object.less_or_equal(&rhs).map(Into::into),
        abi::BinaryOp::Greater => object.greater(&rhs).map(Into::into),
        abi::BinaryOp::GreaterOrEqual => object.greater_or_equal(&rhs).map(Into::into),
        abi::BinaryOp::Equal => object.equal(&rhs).map(Into::into),
        abi::BinaryOp::NotEqual => object.not_equal(&rhs).map(Into::into),
        _ => Err(Error::from(format!("unsupported object binary op: {op:?}"))),
    });

    match result {
        Ok(value) => {
            unsafe { *out = encode_abi_value(value) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_binary_op_assign(
    mut borrow: abi::KObjectBorrowMut,
    op: abi::BinaryOp,
    rhs: abi::KValue,
) -> abi::KotoStatus {
    let rhs = take_value(rhs);

    let result = borrowed_object_mut(&mut borrow).and_then(|object| match op {
        abi::BinaryOp::AddAssign => object.add_assign(&rhs),
        abi::BinaryOp::SubtractAssign => object.subtract_assign(&rhs),
        abi::BinaryOp::MultiplyAssign => object.multiply_assign(&rhs),
        abi::BinaryOp::DivideAssign => object.divide_assign(&rhs),
        abi::BinaryOp::RemainderAssign => object.remainder_assign(&rhs),
        abi::BinaryOp::PowerAssign => object.power_assign(&rhs),
        _ => Err(Error::from(format!(
            "unsupported object binary assign op: {op:?}"
        ))),
    });

    match result {
        Ok(()) => abi::KotoStatus::ok(),
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_make_iterator(
    borrow: abi::KObjectBorrow,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match borrowed_object(&borrow).and_then(|object| object.make_iterator(vm)) {
        Ok(iterator) => {
            unsafe { *out = encode_abi_value(KValue::Iterator(iterator)) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn object_borrow_serialize(
    borrow: abi::KObjectBorrow,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    match borrowed_object(&borrow).and_then(|object| object.serialize()) {
        Ok(value) => {
            unsafe { *out = encode_abi_value(value) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_call_function(
    function: abi::KValue,
    args: *const abi::KValue,
    arg_count: usize,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    let args = clone_args_from_handles(args, arg_count);

    match vm.call_function(clone_value_handle(function), args.as_slice()) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_call_instance_function(
    instance: abi::KValue,
    function: abi::KValue,
    args: *const abi::KValue,
    arg_count: usize,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    let args = clone_args_from_handles(args, arg_count);

    match vm.call_instance_function(
        clone_value_handle(instance),
        clone_value_handle(function),
        args.as_slice(),
    ) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_run_unary_op(
    op: abi::UnaryOp,
    value: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match vm.run_unary_op(unary_op_from_abi(op), clone_value_handle(value)) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_run_binary_op(
    op: abi::BinaryOp,
    lhs: abi::KValue,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match vm.run_binary_op(
        binary_op_from_abi(op),
        clone_value_handle(lhs),
        clone_value_handle(rhs),
    ) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_run_read_op(
    op: abi::ReadOp,
    container: abi::KValue,
    read_arg: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match vm.run_read_op(
        read_op_from_abi(op),
        clone_value_handle(container),
        clone_value_handle(read_arg),
    ) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

unsafe extern "C" fn vm_run_write_op(
    op: abi::WriteOp,
    container: abi::KValue,
    write_arg: abi::KValue,
    write_value: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let vm = match current_vm() {
        Ok(vm) => vm,
        Err(error) => return error_to_status(error),
    };

    match vm.run_write_op(
        write_op_from_abi(op),
        clone_value_handle(container),
        clone_value_handle(write_arg),
        clone_value_handle(write_value),
    ) {
        Ok(result) => {
            unsafe { *out = encode_abi_value(result) };
            abi::KotoStatus::ok()
        }
        Err(error) => error_to_status(error),
    }
}

pub(crate) const HOST_API: abi::KotoHostApiV1 = abi::KotoHostApiV1 {
    abi_major: abi::ABI_MAJOR_VERSION,
    abi_minor: abi::ABI_MINOR_VERSION,
    struct_size: size_of::<abi::KotoHostApiV1>(),
    map_new_with_type,
    map_insert_value,
    map_insert_meta_value,
    native_function_make,
    value_make_null,
    value_make_bool,
    value_make_i64,
    value_make_f64,
    value_make_range,
    string_make,
    tuple_make,
    map_make,
    object_make,
    value_clone,
    value_free,
    value_view_clone,
    value_is_same_instance,
    value_kind,
    value_as_bool,
    value_as_i64,
    value_as_f64,
    value_as_range,
    string_as_slice,
    tuple_len,
    tuple_data,
    tuple_get,
    map_len,
    map_data,
    map_data_get_entry,
    map_key_at,
    map_value_at,
    object_v1,
    object_borrow,
    object_borrow_mut,
    object_borrow_free,
    object_borrow_mut_free,
    object_borrow_type_string,
    object_borrow_named_value,
    object_borrow_named_value_assign,
    object_borrow_iterable_kind,
    object_borrow_iterator_next,
    object_borrow_iterator_next_back,
    object_borrow_display,
    object_borrow_size,
    object_borrow_index,
    object_borrow_index_assign,
    object_borrow_is_callable,
    object_borrow_call,
    object_borrow_unary_op,
    object_borrow_binary_op,
    object_borrow_binary_op_assign,
    object_borrow_make_iterator,
    object_borrow_serialize,
    list_make,
    list_len,
    list_data,
    list_get,
    list_set,
    map_swap_indices,
    map_contains_meta_read,
    map_get_meta_read,
    map_contains_meta_write,
    map_get_meta_write,
    vm_call_function,
    vm_call_instance_function,
    vm_run_unary_op,
    vm_run_binary_op,
    vm_run_read_op,
    vm_run_write_op,
};

pub(crate) fn is_native_import(import_name: &str) -> bool {
    import_name.starts_with("native:")
}

pub(crate) fn resolve_native_module_path(
    import_name: &str,
    current_script_path: Option<&Path>,
) -> Result<PathBuf> {
    let Some(raw_path) = import_name.strip_prefix("native:") else {
        return Err(Error::from("native import is missing the 'native:' prefix"));
    };

    if raw_path.is_empty() {
        return Err(Error::from("native import path is empty"));
    }

    let mut base = if let Some(path) = current_script_path {
        if path.is_file() {
            path.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            path.to_path_buf()
        }
    } else {
        std::env::current_dir().map_err(|e| Error::from(e.to_string()))?
    };

    let path = PathBuf::from(raw_path);
    let candidate = if path.is_absolute() {
        path
    } else {
        base.push(path);
        base
    };

    let mut candidates = vec![candidate.clone()];
    if candidate.extension().is_none() {
        let extension = if cfg!(target_os = "windows") {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        candidates.push(candidate.with_extension(extension));

        if let Some(file_name) = candidate.file_name().and_then(|name| name.to_str())
            && !file_name.starts_with("lib")
        {
            let lib_name = format!("lib{file_name}");
            candidates.push(candidate.with_file_name(lib_name).with_extension(extension));
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return std::fs::canonicalize(&candidate).map_err(|e| Error::from(e.to_string()));
        }
    }

    Err(Error::from(format!(
        "unable to find native module '{}'",
        candidate.display()
    )))
}

pub(crate) fn load_native_module(path: &Path) -> Result<LoadedNativeModule> {
    let library = Arc::new(LoadedLibrary {
        library: unsafe { Library::new(path) }.map_err(|e| Error::from(e.to_string()))?,
    });

    let init: Symbol<'_, abi::KotoPluginInitV1> = unsafe {
        library
            .library
            .get(b"koto_plugin_init_v1")
            .map_err(|e| Error::from(e.to_string()))?
    };

    let mut module = abi::KValue::default();
    let status = with_current_library(library.clone(), || unsafe { init(&HOST_API, &mut module) });

    if status.code != abi::KotoStatusCode::Ok {
        return Err(status_to_error(status));
    }

    if matches!(module.kind, abi::KValueKind::Null) {
        return Err(Error::from("dynamic module returned a null module handle"));
    }

    let exports = take_value(module);
    let exports = match exports {
        KValue::Map(exports) => exports,
        other => {
            return Err(Error::from(format!(
                "dynamic module returned {}, expected a Map",
                other.type_as_string()
            )));
        }
    };

    Ok(LoadedNativeModule { exports, library })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Backend, KObject, derive::*};
    use koto_api::{KotoAccess, KotoObjectOps};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Clone, Copy, KotoCopy, KotoType)]
    #[koto(runtime = crate, use_copy)]
    struct BorrowConflictObject;

    impl KotoAccess<Backend> for BorrowConflictObject {}

    impl KotoObjectOps<Backend> for BorrowConflictObject {}

    fn object_abi_handle(object: &KObject) -> abi::KObject {
        unsafe { object.clone().into_abi() }
    }

    #[test]
    fn plugin_mutable_borrow_blocks_runtime_borrows() {
        let object = KObject::from(BorrowConflictObject);
        let handle = object_abi_handle(&object);
        let plugin_borrow = unsafe { object_borrow_mut(handle) };

        assert!(plugin_borrow.is_valid());
        assert!(object.try_borrow().is_err());
        assert!(object.try_borrow_mut().is_err());

        unsafe {
            object_borrow_mut_free(plugin_borrow);
            drop(take_abi::<KObject>(handle));
        }

        assert!(object.try_borrow().is_ok());
        assert!(object.try_borrow_mut().is_ok());
    }

    #[test]
    fn runtime_mutable_borrow_blocks_plugin_borrows() {
        let object = KObject::from(BorrowConflictObject);
        let runtime_borrow = object.try_borrow_mut().unwrap();
        let handle = object_abi_handle(&object);

        let plugin_borrow = unsafe { object_borrow(handle) };
        let plugin_borrow_mut = unsafe { object_borrow_mut(handle) };

        assert!(!plugin_borrow.is_valid());
        assert!(!plugin_borrow_mut.is_valid());

        drop(runtime_borrow);
        unsafe {
            drop(take_abi::<KObject>(handle));
        }

        let shared_handle = object_abi_handle(&object);
        let plugin_borrow = unsafe { object_borrow(shared_handle) };
        assert!(plugin_borrow.is_valid());
        unsafe {
            object_borrow_free(plugin_borrow);
            drop(take_abi::<KObject>(shared_handle));
        }

        let mutable_handle = object_abi_handle(&object);
        let plugin_borrow_mut = unsafe { object_borrow_mut(mutable_handle) };
        assert!(plugin_borrow_mut.is_valid());
        unsafe {
            object_borrow_mut_free(plugin_borrow_mut);
            drop(take_abi::<KObject>(mutable_handle));
        }
    }

    #[test]
    fn runtime_error_status_preserves_error_kind() {
        let original = Error::new(crate::ErrorKind::UnexpectedType {
            expected: "Number".into(),
            unexpected: KValue::Null,
        });

        let round_tripped = status_to_error(error_to_status(original));

        match round_tripped.error {
            crate::ErrorKind::UnexpectedType {
                expected,
                unexpected,
            } => {
                assert_eq!(expected, "Number");
                assert!(matches!(unexpected, KValue::Null));
            }
            unexpected => panic!("unexpected error kind after round-trip: {unexpected:?}"),
        }
    }

    #[test]
    fn detects_native_import_prefix() {
        assert!(is_native_import("native:/tmp/foo"));
        assert!(!is_native_import("json"));
    }

    #[test]
    fn resolves_relative_native_imports_with_platform_extension() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("koto_native_test_{nonce}"));
        fs::create_dir_all(&temp_dir).unwrap();

        let extension = if cfg!(target_os = "windows") {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        let library_path = temp_dir.join(format!("libsample.{extension}"));
        fs::write(&library_path, []).unwrap();
        let script_path = temp_dir.join("script.koto");
        fs::write(&script_path, "# test").unwrap();

        let resolved = resolve_native_module_path("native:sample", Some(&script_path)).unwrap();
        assert_eq!(resolved, fs::canonicalize(&library_path).unwrap());

        let _ = fs::remove_file(&library_path);
        let _ = fs::remove_file(&script_path);
        let _ = fs::remove_dir(&temp_dir);
    }
}
