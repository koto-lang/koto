use crate::api::{
    BinaryOp, KotoIteratorBuilder, KotoNamedAccess, KotoObjectHandle, KotoObjectOps, KotoVmTrait,
    UnaryOp,
};
use crate::derive::{
    KotoCopy, KotoType, koto_fn, koto_get, koto_get_fallback, koto_get_override, koto_impl,
    koto_method, koto_set, koto_set_fallback, koto_set_override,
};
use crate::*;
use koto_ffi as abi;
use std::{
    alloc::Layout,
    ffi::c_void,
    mem::{align_of, size_of},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct SharedTestObject {
    object_v1: *const abi::KotoPluginObjectV1,
    data: *mut c_void,
    layout: Layout,
    drop_data: abi::ObjectFnDropData,
}

// Safety: the test object is only used as an opaque shared handle. Access to the underlying
// object implementation still goes through the test host callbacks, and the stored pointers are
// immutable after construction.
unsafe impl Send for SharedTestObject {}
// Safety: the test object contains immutable callback/data pointers plus layout metadata, and the
// actual object interaction is synchronized at the host API boundary in these tests.
unsafe impl Sync for SharedTestObject {}

impl Drop for SharedTestObject {
    fn drop(&mut self) {
        unsafe {
            (self.drop_data)(self.data);
            std::alloc::dealloc(self.data as *mut u8, self.layout);
        };
    }
}

struct SharedTestNativeFunction {
    function: abi::KotoPluginFunction,
    user_data: *mut c_void,
    drop_user_data: abi::KotoPluginDrop,
}

unsafe impl Send for SharedTestNativeFunction {}
unsafe impl Sync for SharedTestNativeFunction {}

impl Drop for SharedTestNativeFunction {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) };
    }
}

enum TestValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    NativeFunction(Arc<SharedTestNativeFunction>),
    Object(Arc<SharedTestObject>),
    Iterator(Arc<SharedTestObject>),
}

fn test_borrow_storage_size() -> usize {
    size_of::<[usize; abi::KOBJECT_BORROW_WORDS]>()
}

fn make_test_borrow_token(token: Arc<SharedTestObject>) -> abi::KObjectBorrow {
    assert!(
        size_of::<Arc<SharedTestObject>>() <= test_borrow_storage_size(),
        "test borrow token storage is too small"
    );
    assert!(
        align_of::<Arc<SharedTestObject>>() <= align_of::<abi::KObjectBorrow>(),
        "test borrow token alignment is too large"
    );

    let mut result = abi::KObjectBorrow {
        data: token.data,
        ..Default::default()
    };
    unsafe {
        std::ptr::write(
            result.storage.as_mut_ptr().cast::<Arc<SharedTestObject>>(),
            token,
        );
    }
    result
}

fn make_test_borrow_mut_token(token: Arc<SharedTestObject>) -> abi::KObjectBorrowMut {
    assert!(
        size_of::<Arc<SharedTestObject>>() <= test_borrow_storage_size(),
        "test borrow token storage is too small"
    );
    assert!(
        align_of::<Arc<SharedTestObject>>() <= align_of::<abi::KObjectBorrowMut>(),
        "test borrow token alignment is too large"
    );

    let mut result = abi::KObjectBorrowMut {
        data: token.data,
        ..Default::default()
    };
    unsafe {
        std::ptr::write(
            result.storage.as_mut_ptr().cast::<Arc<SharedTestObject>>(),
            token,
        );
    }
    result
}

fn with_test_host_api<T>(f: impl FnOnce() -> T) -> T {
    with_host_api(&TEST_HOST_API, f)
}

struct EncodedTestArgs(Vec<abi::KValue>);

impl EncodedTestArgs {
    fn new(args: Vec<KValue>) -> Self {
        Self(
            args.into_iter()
                .map(|value| crate::types::encode_value(&TEST_HOST_API, value))
                .collect(),
        )
    }

    fn as_ptr(&self) -> *const abi::KValue {
        self.0.as_ptr()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl Drop for EncodedTestArgs {
    fn drop(&mut self) {
        for arg in self.0.drain(..) {
            unsafe { (TEST_HOST_API.value_free)(arg) };
        }
    }
}

fn with_test_call_context<T>(
    instance: KValue,
    args: Vec<KValue>,
    f: impl FnOnce(&mut CallContext) -> T,
) -> T {
    let encoded_args = EncodedTestArgs::new(args);
    let mut ctx = CallContext::from_abi(
        &TEST_HOST_API,
        KotoVm::from_api(&TEST_HOST_API),
        instance,
        encoded_args.as_ptr(),
        encoded_args.len(),
    );
    f(&mut ctx)
}

fn boxed_test_value(value: TestValue) -> abi::KValue {
    match value {
        TestValue::Null => abi::KValue::null(),
        TestValue::Bool(value) => abi::KValue {
            kind: abi::KValueKind::Bool,
            data: abi::KValueData { bool_value: value },
        },
        TestValue::Number(value) => abi::KValue {
            kind: abi::KValueKind::I64,
            data: abi::KValueData { i64_value: value },
        },
        TestValue::String(value) => abi::KValue {
            kind: abi::KValueKind::String,
            data: abi::KValueData {
                string_value: abi::KString {
                    kind: abi::KStringKind::Full,
                    data: abi::KStringData {
                        full: Box::into_raw(Box::new(value)) as *mut c_void,
                    },
                },
            },
        },
        TestValue::NativeFunction(value) => abi::KValue {
            kind: abi::KValueKind::NativeFunction,
            data: abi::KValueData {
                native_function_value: abi::OpaqueHandle {
                    data: Box::into_raw(Box::new(TestValue::NativeFunction(value))) as *mut c_void,
                    metadata: std::ptr::null_mut(),
                },
            },
        },
        TestValue::Object(value) => abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: abi::KObject {
                    data: Box::into_raw(Box::new(TestValue::Object(value))) as *mut c_void,
                    metadata: std::ptr::null_mut(),
                },
            },
        },
        TestValue::Iterator(value) => abi::KValue {
            kind: abi::KValueKind::Iterator,
            data: abi::KValueData {
                iterator_value: abi::OpaqueHandle {
                    data: Box::into_raw(Box::new(TestValue::Iterator(value))) as *mut c_void,
                    metadata: std::ptr::null_mut(),
                },
            },
        },
    }
}

fn object_handle(value: abi::KValue) -> Option<&'static Arc<SharedTestObject>> {
    match value.kind {
        abi::KValueKind::Object => {
            match unsafe { &*(value.data.object_value.data as *const TestValue) } {
                TestValue::Object(object) => Some(object),
                _ => None,
            }
        }
        _ => None,
    }
}

fn native_function_handle(value: abi::KValue) -> Option<&'static Arc<SharedTestNativeFunction>> {
    match value.kind {
        abi::KValueKind::NativeFunction => {
            match unsafe { &*(value.data.native_function_value.data as *const TestValue) } {
                TestValue::NativeFunction(function) => Some(function),
                _ => None,
            }
        }
        _ => None,
    }
}

fn iterator_handle(value: abi::KValue) -> Option<&'static Arc<SharedTestObject>> {
    match value.kind {
        abi::KValueKind::Iterator => {
            match unsafe { &*(value.data.iterator_value.data as *const TestValue) } {
                TestValue::Iterator(object) => Some(object),
                _ => None,
            }
        }
        _ => None,
    }
}

fn shared_object(object: abi::KObject) -> Option<&'static Arc<SharedTestObject>> {
    object_handle(abi::KValue {
        kind: abi::KValueKind::Object,
        data: abi::KValueData {
            object_value: object,
        },
    })
}

fn borrowed_shared_object(borrow: abi::KObjectBorrow) -> Option<&'static Arc<SharedTestObject>> {
    if borrow.is_valid() {
        Some(unsafe { &*borrow.storage.as_ptr().cast::<Arc<SharedTestObject>>() })
    } else {
        None
    }
}

fn borrowed_shared_object_mut(
    borrow: abi::KObjectBorrowMut,
) -> Option<&'static Arc<SharedTestObject>> {
    if borrow.is_valid() {
        Some(unsafe { &*borrow.storage.as_ptr().cast::<Arc<SharedTestObject>>() })
    } else {
        None
    }
}

fn with_shared_object_value<T>(
    shared: &Arc<SharedTestObject>,
    f: impl FnOnce(abi::KObject) -> T,
) -> T {
    let object = abi::KObject {
        data: Box::into_raw(Box::new(TestValue::Object(shared.clone()))) as *mut c_void,
        metadata: std::ptr::null_mut(),
    };
    let result = f(object);
    let _ = unsafe { Box::from_raw(object.data as *mut TestValue) };
    result
}

fn test_error_status(message: &str) -> abi::KotoStatus {
    abi::KotoStatus {
        code: abi::KotoStatusCode::Error,
        error: std::ptr::null_mut(),
        clone_error: std::ptr::null(),
        free_error: std::ptr::null(),
        is_unimplemented: false,
        message: std::ffi::CString::new(message).unwrap().into_raw(),
    }
}

static RUNTIME_ERROR_CLONES: AtomicUsize = AtomicUsize::new(0);
static RUNTIME_ERROR_FREES: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn clone_test_runtime_error(error: *mut c_void) -> *mut c_void {
    RUNTIME_ERROR_CLONES.fetch_add(1, Ordering::SeqCst);
    let value = unsafe { *(error.cast::<usize>()) };
    Box::into_raw(Box::new(value)).cast()
}

unsafe extern "C" fn free_test_runtime_error(error: *mut c_void) {
    RUNTIME_ERROR_FREES.fetch_add(1, Ordering::SeqCst);
    if !error.is_null() {
        unsafe { drop(Box::from_raw(error.cast::<usize>())) };
    }
}

unsafe extern "C" fn unsupported_map_new_with_type(_type_name: abi::KStringSlice) -> abi::KMap {
    abi::KMap::default()
}

unsafe extern "C" fn unsupported_map_insert_value(
    _map: abi::KMap,
    _key: abi::KStringSlice,
    _value: abi::KValue,
) {
}

unsafe extern "C" fn unsupported_map_insert_meta_value(
    _map: abi::KMap,
    _key: abi::MetaKey,
    _value: abi::KValue,
) {
}

unsafe extern "C" fn test_native_function_make(
    function: abi::KotoPluginFunction,
    user_data: *mut c_void,
    drop_user_data: abi::KotoPluginDrop,
) -> abi::OpaqueHandle {
    unsafe {
        boxed_test_value(TestValue::NativeFunction(Arc::new(
            SharedTestNativeFunction {
                function,
                user_data,
                drop_user_data,
            },
        )))
        .data
        .native_function_value
    }
}

unsafe extern "C" fn test_value_make_null() -> abi::KValue {
    boxed_test_value(TestValue::Null)
}

unsafe extern "C" fn unsupported_value_make_bool(_value: bool) -> abi::KValue {
    boxed_test_value(TestValue::Bool(_value))
}

unsafe extern "C" fn unsupported_value_make_i64(_value: i64) -> abi::KValue {
    boxed_test_value(TestValue::Number(_value))
}

unsafe extern "C" fn unsupported_value_make_f64(_value: f64) -> abi::KValue {
    unsafe { test_value_make_null() }
}

unsafe extern "C" fn unsupported_value_make_range(_value: abi::KotoRange) -> abi::KValue {
    unsafe { test_value_make_null() }
}

unsafe extern "C" fn unsupported_string_make(_value: abi::KStringSlice) -> abi::KString {
    let bytes = if _value.ptr.is_null() || _value.len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(_value.ptr, _value.len) }
    };
    abi::KString {
        kind: abi::KStringKind::Full,
        data: abi::KStringData {
            full: Box::into_raw(Box::new(
                String::from_utf8(bytes.to_vec()).expect("test strings should be valid utf-8"),
            )) as *mut c_void,
        },
    }
}

unsafe extern "C" fn unsupported_tuple_make(
    _values: *const abi::KValue,
    _len: usize,
) -> abi::KTuple {
    abi::KTuple::default()
}

unsafe extern "C" fn unsupported_list_make(_values: *const abi::KValue, _len: usize) -> abi::KList {
    abi::KList::default()
}

unsafe extern "C" fn unsupported_map_make(
    _entries: *const abi::KotoMapEntry,
    _len: usize,
) -> abi::KMap {
    abi::KMap::default()
}

unsafe extern "C" fn test_object_make(
    object_v1: *const abi::KotoPluginObjectV1,
    object_data: abi::KotoObjectDataV1,
) -> abi::KObject {
    let layout = Layout::from_size_align(object_data.size, object_data.align).unwrap();
    let data = unsafe { std::alloc::alloc(layout) as *mut c_void };
    unsafe { (object_data.init)(data, object_data.source) };

    abi::KObject {
        data: Box::into_raw(Box::new(TestValue::Object(Arc::new(SharedTestObject {
            object_v1,
            data,
            layout,
            drop_data: object_data.drop,
        })))) as *mut c_void,
        metadata: std::ptr::null_mut(),
    }
}

unsafe extern "C" fn test_value_clone(value: abi::KValue) -> abi::KValue {
    match value.kind {
        abi::KValueKind::Null => unsafe { test_value_make_null() },
        abi::KValueKind::Bool => {
            boxed_test_value(TestValue::Bool(unsafe { value.data.bool_value }))
        }
        abi::KValueKind::I64 => {
            boxed_test_value(TestValue::Number(unsafe { value.data.i64_value }))
        }
        abi::KValueKind::String => {
            let value = unsafe { &*(value.data.string_value.data.full as *const String) };
            boxed_test_value(TestValue::String(value.clone()))
        }
        abi::KValueKind::NativeFunction => boxed_test_value(TestValue::NativeFunction(
            native_function_handle(value).unwrap().clone(),
        )),
        abi::KValueKind::Object => {
            boxed_test_value(TestValue::Object(object_handle(value).unwrap().clone()))
        }
        abi::KValueKind::Iterator => {
            boxed_test_value(TestValue::Iterator(iterator_handle(value).unwrap().clone()))
        }
        abi::KValueKind::Function => abi::KValue::default(),
        _ => abi::KValue::default(),
    }
}

unsafe extern "C" fn unsupported_value_view_clone(_value: abi::KValueView) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn test_value_free(value: abi::KValue) {
    match value.kind {
        abi::KValueKind::String => {
            let handle = unsafe { value.data.string_value.data.full };
            if !handle.is_null() {
                let _ = unsafe { Box::from_raw(handle as *mut String) };
            }
        }
        abi::KValueKind::Object => {
            let handle = unsafe { value.data.object_value.data };
            if !handle.is_null() {
                let _ = unsafe { Box::from_raw(handle as *mut TestValue) };
            }
        }
        abi::KValueKind::Function => {}
        abi::KValueKind::NativeFunction => {
            let handle = unsafe { value.data.native_function_value.data };
            if !handle.is_null() {
                let _ = unsafe { Box::from_raw(handle as *mut TestValue) };
            }
        }
        abi::KValueKind::Iterator => {
            let handle = unsafe { value.data.iterator_value.data };
            if !handle.is_null() {
                let _ = unsafe { Box::from_raw(handle as *mut TestValue) };
            }
        }
        _ if value.is_handle() => {
            let handle = unsafe { value.data.handle };
            if !handle.is_null() {
                let _ = unsafe { Box::from_raw(handle as *mut TestValue) };
            }
        }
        _ => {}
    }
}

unsafe extern "C" fn test_value_is_same_instance(a: abi::KValue, b: abi::KValue) -> bool {
    if matches!(a.kind, abi::KValueKind::Object) && matches!(b.kind, abi::KValueKind::Object) {
        let a_object = object_handle(a).unwrap();
        let b_object = object_handle(b).unwrap();
        a_object.object_v1 == b_object.object_v1 && Arc::ptr_eq(a_object, b_object)
    } else {
        false
    }
}

unsafe extern "C" fn test_value_kind(value: abi::KValue) -> abi::KValueKind {
    value.kind
}

unsafe extern "C" fn unsupported_value_as_bool(_value: abi::KValue) -> bool {
    matches!(_value.kind, abi::KValueKind::Bool)
        .then(|| unsafe { _value.data.bool_value })
        .unwrap_or(false)
}

unsafe extern "C" fn unsupported_value_as_i64(_value: abi::KValue) -> i64 {
    matches!(_value.kind, abi::KValueKind::I64)
        .then(|| unsafe { _value.data.i64_value })
        .unwrap_or_default()
}

unsafe extern "C" fn unsupported_value_as_f64(_value: abi::KValue) -> f64 {
    matches!(_value.kind, abi::KValueKind::I64)
        .then(|| unsafe { _value.data.i64_value as f64 })
        .unwrap_or_default()
}

unsafe extern "C" fn unsupported_value_as_range(_value: abi::KValue) -> abi::KotoRange {
    abi::KotoRange::default()
}

unsafe extern "C" fn unsupported_string_as_slice(_value: abi::KString) -> abi::KStringSlice {
    match _value.kind {
        abi::KStringKind::Full => {
            let value = unsafe { &*(_value.data.full as *const String) };
            abi::KStringSlice {
                ptr: value.as_ptr(),
                len: value.len(),
            }
        }
        _ => abi::KStringSlice::default(),
    }
}

unsafe extern "C" fn unsupported_tuple_len(_tuple: abi::KTuple) -> usize {
    0
}

unsafe extern "C" fn unsupported_tuple_data(_tuple: abi::KTuple) -> abi::KValueSlice {
    abi::KValueSlice::default()
}

unsafe extern "C" fn unsupported_list_len(_list: abi::KList) -> usize {
    0
}

unsafe extern "C" fn unsupported_list_data(_list: abi::KList) -> abi::KValueSlice {
    abi::KValueSlice::default()
}

unsafe extern "C" fn unsupported_list_get(_list: abi::KList, _index: usize) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn unsupported_list_set(
    _list: abi::KList,
    _index: usize,
    _item: abi::KValue,
) -> abi::KotoStatus {
    test_error_status("list_set is unsupported in tests")
}

unsafe extern "C" fn unsupported_map_data(_map: abi::KMap) -> abi::KMapData {
    abi::KMapData::default()
}

unsafe extern "C" fn unsupported_map_data_get_entry(
    _map: abi::KMapData,
    _index: usize,
) -> abi::KMapEntryView {
    abi::KMapEntryView::default()
}

unsafe extern "C" fn unsupported_tuple_get(_tuple: abi::KTuple, _index: usize) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn unsupported_map_len(_value: abi::KMap) -> usize {
    0
}

unsafe extern "C" fn unsupported_map_key_at(_value: abi::KMap, _index: usize) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn unsupported_map_value_at(_value: abi::KMap, _index: usize) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn unsupported_map_swap_indices(
    _value: abi::KMap,
    _a: usize,
    _b: usize,
) -> abi::KotoStatus {
    test_error_status("map_swap_indices is unsupported in tests")
}

unsafe extern "C" fn unsupported_map_contains_meta_read(
    _value: abi::KMap,
    _op: abi::ReadOp,
) -> bool {
    false
}

unsafe extern "C" fn unsupported_map_get_meta_read(
    _value: abi::KMap,
    _op: abi::ReadOp,
) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn unsupported_map_contains_meta_write(
    _value: abi::KMap,
    _op: abi::WriteOp,
) -> bool {
    false
}

unsafe extern "C" fn unsupported_map_get_meta_write(
    _value: abi::KMap,
    _op: abi::WriteOp,
) -> abi::KValue {
    abi::KValue::default()
}

unsafe extern "C" fn test_object_v1(object: abi::KObject) -> *const abi::KotoPluginObjectV1 {
    shared_object(object)
        .map(|object| object.object_v1)
        .unwrap_or(std::ptr::null())
}

unsafe extern "C" fn test_object_type_string(object: abi::KObject) -> abi::KString {
    let Some(shared) = shared_object(object) else {
        return abi::KString::default();
    };
    let object_v1 = unsafe { &*shared.object_v1 };
    unsafe { unsupported_string_make((object_v1.type_string)(&TEST_HOST_API, object)) }
}

unsafe extern "C" fn test_object_named_value(
    object: abi::KObject,
    key: abi::KStringSlice,
    out: *mut abi::KValue,
    out_found: *mut bool,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_named_value is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.named_value)(&TEST_HOST_API, object, key, out, out_found) }
}

unsafe extern "C" fn test_object_named_value_assign(
    object: abi::KObject,
    key: abi::KStringSlice,
    value: abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_named_value_assign is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.named_value_assign)(&TEST_HOST_API, object, key, value) }
}

unsafe extern "C" fn test_object_display(object: abi::KObject) -> abi::KString {
    let Some(shared) = shared_object(object) else {
        return abi::KString::default();
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    let mut out = abi::KValue::default();
    let status = unsafe { (object_v1.display)(&TEST_HOST_API, object, &mut out) };
    if status.code != abi::KotoStatusCode::Ok {
        return abi::KString::default();
    }

    if matches!(out.kind, abi::KValueKind::String) {
        unsafe { out.data.string_value }
    } else {
        abi::KString::default()
    }
}

unsafe extern "C" fn test_object_iterable_kind(object: abi::KObject) -> abi::IterableKind {
    let Some(shared) = shared_object(object) else {
        return abi::IterableKind::NotIterable;
    };
    let object_v1 = unsafe { &*shared.object_v1 };
    unsafe { (object_v1.iterable_kind)(&TEST_HOST_API, object) }
}

unsafe extern "C" fn test_object_iterator_next(
    object: abi::KObject,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_iterator_next is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.iterator_next)(&TEST_HOST_API, object, out, out_has_value) }
}

unsafe extern "C" fn test_object_iterator_next_back(
    object: abi::KObject,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_iterator_next_back is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.iterator_next_back)(&TEST_HOST_API, object, out, out_has_value) }
}

unsafe extern "C" fn test_object_borrow(object: abi::KObject) -> abi::KObjectBorrow {
    if object.data.is_null() {
        abi::KObjectBorrow::default()
    } else {
        object_handle(abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: object,
            },
        })
        .map(|object| make_test_borrow_token(object.clone()))
        .unwrap_or_default()
    }
}

unsafe extern "C" fn test_object_borrow_mut(object: abi::KObject) -> abi::KObjectBorrowMut {
    if object.data.is_null() {
        abi::KObjectBorrowMut::default()
    } else {
        object_handle(abi::KValue {
            kind: abi::KValueKind::Object,
            data: abi::KValueData {
                object_value: object,
            },
        })
        .map(|object| make_test_borrow_mut_token(object.clone()))
        .unwrap_or_default()
    }
}

unsafe extern "C" fn test_object_borrow_free(borrow: abi::KObjectBorrow) {
    if borrow.is_valid() {
        unsafe {
            std::ptr::drop_in_place(borrow.storage.as_ptr().cast::<Arc<SharedTestObject>>()
                as *mut Arc<SharedTestObject>);
        }
    }
}

unsafe extern "C" fn test_object_borrow_mut_free(borrow: abi::KObjectBorrowMut) {
    if borrow.is_valid() {
        unsafe {
            std::ptr::drop_in_place(borrow.storage.as_ptr().cast::<Arc<SharedTestObject>>()
                as *mut Arc<SharedTestObject>);
        }
    }
}

unsafe extern "C" fn test_object_borrow_type_string(borrow: abi::KObjectBorrow) -> abi::KString {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe { test_object_type_string(object) })
        })
        .unwrap_or_default()
}

unsafe extern "C" fn test_object_borrow_named_value(
    borrow: abi::KObjectBorrow,
    key: abi::KStringSlice,
    out: *mut abi::KValue,
    out_found: *mut bool,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_named_value(object, key, out, out_found)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_named_value is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_named_value_assign(
    borrow: abi::KObjectBorrowMut,
    key: abi::KStringSlice,
    value: abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_named_value_assign(object, key, value)
            })
        })
        .unwrap_or_else(|| {
            test_error_status("object_borrow_named_value_assign is unsupported in tests")
        })
}

unsafe extern "C" fn test_object_borrow_iterable_kind(
    borrow: abi::KObjectBorrow,
) -> abi::IterableKind {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_iterable_kind(object)
            })
        })
        .unwrap_or(abi::IterableKind::NotIterable)
}

unsafe extern "C" fn test_object_borrow_iterator_next(
    borrow: abi::KObjectBorrowMut,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_iterator_next(object, out, out_has_value)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_iterator_next is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_iterator_next_back(
    borrow: abi::KObjectBorrowMut,
    out: *mut abi::KValue,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_iterator_next_back(object, out, out_has_value)
            })
        })
        .unwrap_or_else(|| {
            test_error_status("object_borrow_iterator_next_back is unsupported in tests")
        })
}

unsafe extern "C" fn test_object_borrow_display(borrow: abi::KObjectBorrow) -> abi::KString {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe { test_object_display(object) })
        })
        .unwrap_or_default()
}

unsafe extern "C" fn test_object_borrow_size(
    borrow: abi::KObjectBorrow,
    out: *mut usize,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_size(object, out, out_has_value)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_size is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_index(
    borrow: abi::KObjectBorrow,
    index: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_index(object, index, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_index is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_index_assign(
    borrow: abi::KObjectBorrowMut,
    index: abi::KValue,
    value: abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_index_assign(object, index, value)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_index_assign is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_is_callable(
    borrow: abi::KObjectBorrow,
    out: *mut bool,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_is_callable(object, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_is_callable is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_call(
    borrow: abi::KObjectBorrowMut,
    ctx: abi::CallContext,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_call(object, ctx, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_call is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_unary_op(
    borrow: abi::KObjectBorrow,
    op: abi::UnaryOp,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_unary_op(object, op, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_unary_op is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_binary_op(
    borrow: abi::KObjectBorrow,
    op: abi::BinaryOp,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_binary_op(object, op, rhs, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_binary_op is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_binary_op_assign(
    borrow: abi::KObjectBorrowMut,
    op: abi::BinaryOp,
    rhs: abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object_mut(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_binary_op_assign(object, op, rhs)
            })
        })
        .unwrap_or_else(|| {
            test_error_status("object_borrow_binary_op_assign is unsupported in tests")
        })
}

unsafe extern "C" fn test_object_borrow_make_iterator(
    borrow: abi::KObjectBorrow,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_make_iterator(object, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_make_iterator is unsupported in tests"))
}

unsafe extern "C" fn test_object_borrow_serialize(
    borrow: abi::KObjectBorrow,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    borrowed_shared_object(borrow)
        .map(|shared| {
            with_shared_object_value(shared, |object| unsafe {
                test_object_serialize(object, out)
            })
        })
        .unwrap_or_else(|| test_error_status("object_borrow_serialize is unsupported in tests"))
}

unsafe extern "C" fn test_object_size(
    object: abi::KObject,
    out: *mut usize,
    out_has_value: *mut bool,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_size is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.size)(&TEST_HOST_API, object, out, out_has_value) }
}

unsafe extern "C" fn test_object_index(
    object: abi::KObject,
    index: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_index is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.index)(&TEST_HOST_API, object, index, out) }
}

unsafe extern "C" fn test_object_index_assign(
    object: abi::KObject,
    index: abi::KValue,
    value: abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_index_assign is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.index_assign)(&TEST_HOST_API, object, index, value) }
}

unsafe extern "C" fn test_object_is_callable(
    object: abi::KObject,
    out: *mut bool,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_is_callable is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };
    unsafe { (object_v1.is_callable)(&TEST_HOST_API, object, out) }
}

unsafe extern "C" fn test_object_call(
    object: abi::KObject,
    ctx: abi::CallContext,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_call is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.call)(&TEST_HOST_API, object, ctx, out) }
}

unsafe extern "C" fn test_object_unary_op(
    object: abi::KObject,
    op: abi::UnaryOp,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_unary_op is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.unary_op)(&TEST_HOST_API, object, op, out) }
}

unsafe extern "C" fn test_object_binary_op(
    object: abi::KObject,
    op: abi::BinaryOp,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_binary_op is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.binary_op)(&TEST_HOST_API, object, op, rhs, out) }
}

unsafe extern "C" fn test_object_binary_op_assign(
    object: abi::KObject,
    op: abi::BinaryOp,
    rhs: abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_binary_op_assign is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    unsafe { (object_v1.binary_op_assign)(&TEST_HOST_API, object, op, rhs) }
}

unsafe extern "C" fn test_object_make_iterator(
    object: abi::KObject,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let Some(shared) = shared_object(object) else {
        return test_error_status("object_make_iterator is unsupported in tests");
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    match unsafe { test_object_iterable_kind(object) } {
        abi::IterableKind::ForwardIterator | abi::IterableKind::BidirectionalIterator => {
            unsafe { *out = boxed_test_value(TestValue::Iterator(shared.clone())) };
            abi::KotoStatus::ok()
        }
        _ => unsafe { (object_v1.make_iterator)(&TEST_HOST_API, object, out) },
    }
}

unsafe extern "C" fn test_object_serialize(
    _object: abi::KObject,
    _out: *mut abi::KValue,
) -> abi::KotoStatus {
    test_error_status("object_serialize is unsupported in tests")
}

unsafe extern "C" fn test_vm_call_function(
    function: abi::KValue,
    args: *const abi::KValue,
    arg_count: usize,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    unsafe {
        test_vm_call_instance_function(test_value_make_null(), function, args, arg_count, out)
    }
}

unsafe extern "C" fn test_vm_call_instance_function(
    instance: abi::KValue,
    function: abi::KValue,
    args: *const abi::KValue,
    arg_count: usize,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    match object_handle(function) {
        Some(object_handle) => {
            let object = unsafe { function.data.object_value };
            unsafe {
                ((&*object_handle.object_v1).call)(
                    &TEST_HOST_API,
                    object,
                    abi::CallContext {
                        instance,
                        args,
                        arg_count,
                    },
                    out,
                )
            }
        }
        None => match native_function_handle(function) {
            Some(function) => unsafe {
                (function.function)(
                    &TEST_HOST_API,
                    abi::CallContext {
                        instance,
                        args,
                        arg_count,
                    },
                    function.user_data,
                    out,
                )
            },
            None => test_error_status("vm_call_instance_function expected a callable in tests"),
        },
    }
}

unsafe extern "C" fn test_vm_run_unary_op(
    op: abi::UnaryOp,
    value: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let (object, shared) = match value.kind {
        abi::KValueKind::Object => {
            let object = unsafe { value.data.object_value };
            match shared_object(object) {
                Some(shared) => (object, shared),
                None => {
                    return test_error_status("vm_run_unary_op expected a shared object in tests");
                }
            }
        }
        abi::KValueKind::Iterator => {
            let Some(shared) = iterator_handle(value) else {
                return test_error_status("vm_run_unary_op expected a shared iterator in tests");
            };
            let object = abi::KObject {
                data: Box::into_raw(Box::new(TestValue::Object(shared.clone()))) as *mut c_void,
                metadata: std::ptr::null_mut(),
            };
            (object, shared)
        }
        _ => return test_error_status("vm_run_unary_op expected an object in tests"),
    };
    let object_v1 = unsafe { &*shared.object_v1 };

    match op {
        abi::UnaryOp::Iterator => match unsafe { test_object_iterable_kind(object) } {
            abi::IterableKind::ForwardIterator | abi::IterableKind::BidirectionalIterator => {
                unsafe { *out = boxed_test_value(TestValue::Iterator(shared.clone())) };
                abi::KotoStatus::ok()
            }
            _ => unsafe { (object_v1.make_iterator)(&TEST_HOST_API, object, out) },
        },
        abi::UnaryOp::Next => {
            let mut has_value = false;
            let status =
                unsafe { (object_v1.iterator_next)(&TEST_HOST_API, object, out, &mut has_value) };
            if status.code != abi::KotoStatusCode::Ok || has_value {
                status
            } else {
                unsafe { *out = abi::KValue::null() };
                abi::KotoStatus::ok()
            }
        }
        abi::UnaryOp::NextBack => {
            let mut has_value = false;
            let status = unsafe {
                (object_v1.iterator_next_back)(&TEST_HOST_API, object, out, &mut has_value)
            };
            if status.code != abi::KotoStatusCode::Ok || has_value {
                status
            } else {
                unsafe { *out = abi::KValue::null() };
                abi::KotoStatus::ok()
            }
        }
        _ => test_error_status("vm_run_unary_op is unsupported in tests"),
    }
}

unsafe extern "C" fn test_vm_run_binary_op(
    op: abi::BinaryOp,
    lhs: abi::KValue,
    rhs: abi::KValue,
    out: *mut abi::KValue,
) -> abi::KotoStatus {
    let lhs = if matches!(lhs.kind, abi::KValueKind::I64) {
        unsafe { lhs.data.i64_value }
    } else {
        return test_error_status("expected lhs number");
    };
    let rhs = if matches!(rhs.kind, abi::KValueKind::I64) {
        unsafe { rhs.data.i64_value }
    } else {
        return test_error_status("expected rhs number");
    };

    let result = match op {
        abi::BinaryOp::Add => lhs + rhs,
        _ => return test_error_status("unsupported binary op"),
    };

    unsafe { *out = boxed_test_value(TestValue::Number(result)) };
    abi::KotoStatus::ok()
}

unsafe extern "C" fn unsupported_vm_run_read_op(
    _op: abi::ReadOp,
    _container: abi::KValue,
    _read_arg: abi::KValue,
    _out: *mut abi::KValue,
) -> abi::KotoStatus {
    test_error_status("vm_run_read_op is unsupported in tests")
}

unsafe extern "C" fn unsupported_vm_run_write_op(
    _op: abi::WriteOp,
    _container: abi::KValue,
    _write_arg: abi::KValue,
    _write_value: abi::KValue,
    _out: *mut abi::KValue,
) -> abi::KotoStatus {
    test_error_status("vm_run_write_op is unsupported in tests")
}

const TEST_HOST_API: abi::KotoHostApiV1 = abi::KotoHostApiV1 {
    abi_major: abi::ABI_MAJOR_VERSION,
    abi_minor: abi::ABI_MINOR_VERSION,
    struct_size: std::mem::size_of::<abi::KotoHostApiV1>(),
    map_new_with_type: unsupported_map_new_with_type,
    map_insert_value: unsupported_map_insert_value,
    map_insert_meta_value: unsupported_map_insert_meta_value,
    native_function_make: test_native_function_make,
    value_make_null: test_value_make_null,
    value_make_bool: unsupported_value_make_bool,
    value_make_i64: unsupported_value_make_i64,
    value_make_f64: unsupported_value_make_f64,
    value_make_range: unsupported_value_make_range,
    string_make: unsupported_string_make,
    tuple_make: unsupported_tuple_make,
    map_make: unsupported_map_make,
    object_make: test_object_make,
    value_clone: test_value_clone,
    value_free: test_value_free,
    value_view_clone: unsupported_value_view_clone,
    value_is_same_instance: test_value_is_same_instance,
    value_kind: test_value_kind,
    value_as_bool: unsupported_value_as_bool,
    value_as_i64: unsupported_value_as_i64,
    value_as_f64: unsupported_value_as_f64,
    value_as_range: unsupported_value_as_range,
    string_as_slice: unsupported_string_as_slice,
    tuple_len: unsupported_tuple_len,
    tuple_data: unsupported_tuple_data,
    tuple_get: unsupported_tuple_get,
    list_make: unsupported_list_make,
    list_len: unsupported_list_len,
    list_data: unsupported_list_data,
    list_get: unsupported_list_get,
    list_set: unsupported_list_set,
    map_len: unsupported_map_len,
    map_data: unsupported_map_data,
    map_data_get_entry: unsupported_map_data_get_entry,
    map_key_at: unsupported_map_key_at,
    map_value_at: unsupported_map_value_at,
    map_swap_indices: unsupported_map_swap_indices,
    map_contains_meta_read: unsupported_map_contains_meta_read,
    map_get_meta_read: unsupported_map_get_meta_read,
    map_contains_meta_write: unsupported_map_contains_meta_write,
    map_get_meta_write: unsupported_map_get_meta_write,
    object_v1: test_object_v1,
    object_borrow: test_object_borrow,
    object_borrow_mut: test_object_borrow_mut,
    object_borrow_free: test_object_borrow_free,
    object_borrow_mut_free: test_object_borrow_mut_free,
    object_borrow_type_string: test_object_borrow_type_string,
    object_borrow_named_value: test_object_borrow_named_value,
    object_borrow_named_value_assign: test_object_borrow_named_value_assign,
    object_borrow_iterable_kind: test_object_borrow_iterable_kind,
    object_borrow_iterator_next: test_object_borrow_iterator_next,
    object_borrow_iterator_next_back: test_object_borrow_iterator_next_back,
    object_borrow_display: test_object_borrow_display,
    object_borrow_size: test_object_borrow_size,
    object_borrow_index: test_object_borrow_index,
    object_borrow_index_assign: test_object_borrow_index_assign,
    object_borrow_is_callable: test_object_borrow_is_callable,
    object_borrow_call: test_object_borrow_call,
    object_borrow_unary_op: test_object_borrow_unary_op,
    object_borrow_binary_op: test_object_borrow_binary_op,
    object_borrow_binary_op_assign: test_object_borrow_binary_op_assign,
    object_borrow_make_iterator: test_object_borrow_make_iterator,
    object_borrow_serialize: test_object_borrow_serialize,
    vm_call_function: test_vm_call_function,
    vm_call_instance_function: test_vm_call_instance_function,
    vm_run_unary_op: test_vm_run_unary_op,
    vm_run_binary_op: test_vm_run_binary_op,
    vm_run_read_op: unsupported_vm_run_read_op,
    vm_run_write_op: unsupported_vm_run_write_op,
};

#[derive(Clone, Debug, KotoType, KotoCopy)]
#[koto(runtime = crate)]
struct Example {
    value: i64,
}

#[koto_impl(runtime = crate)]
impl Example {
    #[koto_get]
    fn value(&self) -> i64 {
        self.value
    }

    #[koto_get(alias = "double")]
    fn doubled(&self) -> i64 {
        self.value * 2
    }

    #[koto_get_override]
    fn override_get(&self, key: &KString) -> Option<KValue> {
        (key == "override").then_some("override".into())
    }

    #[koto_get_fallback]
    fn fallback_get(&self, key: &KString) -> Option<KValue> {
        (key == "fallback").then_some("fallback".into())
    }

    #[koto_set]
    fn set_value(&mut self, value: &KValue) -> Result<()> {
        match value {
            KValue::Number(number) => {
                self.value = (*number).into();
                Ok(())
            }
            unexpected => unexpected_type("a Number", unexpected),
        }
    }

    #[koto_set_override]
    fn override_set(&mut self, key: &KString, value: &KValue) -> Result<bool> {
        if key == "override" {
            match value {
                KValue::Number(number) => {
                    self.value = i64::from(*number) * 2;
                    Ok(true)
                }
                unexpected => unexpected_type("a Number", unexpected),
            }
        } else {
            Ok(false)
        }
    }

    #[koto_set_fallback]
    fn fallback_set(&mut self, key: &KString, value: &KValue) -> Result<()> {
        if key == "fallback" {
            match value {
                KValue::Number(number) => {
                    self.value = i64::from(*number) * 3;
                    Ok(())
                }
                unexpected => unexpected_type("a Number", unexpected),
            }
        } else {
            runtime_error!("unexpected key: {key}")
        }
    }

    #[koto_method]
    fn same_value(&self, other: &Example) -> bool {
        self.value == other.value
    }

    #[koto_method]
    fn copy_value_from(&mut self, other: &Example) -> &mut Self {
        self.value = other.value;
        self
    }
}

impl KotoObjectOps<Backend> for Example {}

koto_fn! {
    runtime = crate;

    fn add_numbers(a: i64, b: i64) -> i64 {
        a + b
    }

    fn examples_match(a: &Example, b: &Example) -> bool {
        a.value == b.value
    }

    fn add_with_vm(a: i64, vm: &mut KotoVm) -> Result<KValue> {
        vm.run_binary_op(BinaryOp::Add, a.into(), 1.into())
    }
}

#[test]
fn plugin_object_downcasts_work() {
    with_test_host_api(|| {
        let object = KObject::from(Example { value: 99 });

        assert!(object.is_a::<Example>());
        assert_eq!(object.cast::<Example>().unwrap().value, 99);
        assert_eq!(object.try_borrow().unwrap().type_string(), "Example");
    });
}

#[test]
fn plugin_koto_impl_supports_named_values() {
    with_test_host_api(|| {
        let example = KObject::from(Example { value: 21 });
        let borrowed = example.try_borrow().unwrap();

        assert_number(borrowed.named_value("value").unwrap(), 21);
        assert_number(borrowed.named_value("double").unwrap(), 42);
        assert_string(borrowed.named_value("override").unwrap(), "override");
        assert_string(borrowed.named_value("fallback").unwrap(), "fallback");
        assert!(borrowed.named_value("missing").unwrap().is_none());
    });
}

#[test]
fn plugin_object_borrows_support_direct_named_access() {
    with_test_host_api(|| {
        let example = KObject::from(Example { value: 21 });

        let borrowed = example.try_borrow().unwrap();
        assert_eq!(borrowed.type_string(), "Example");
        assert_number(borrowed.named_value("value").unwrap(), 21);

        let mut borrowed = example.try_borrow_mut().unwrap();
        borrowed.named_value_assign("value", &42.into()).unwrap();
        assert_eq!(example.cast::<Example>().unwrap().value, 42);
    });
}

#[test]
fn plugin_koto_impl_supports_named_value_assignment() {
    with_test_host_api(|| {
        let example = KObject::from(Example { value: 0 });

        {
            let mut borrowed = example.try_borrow_mut().unwrap();
            borrowed.named_value_assign("value", &21.into()).unwrap();
        }
        assert_eq!(example.cast::<Example>().unwrap().value, 21);

        {
            let mut borrowed = example.try_borrow_mut().unwrap();
            borrowed.named_value_assign("override", &3.into()).unwrap();
        }
        assert_eq!(example.cast::<Example>().unwrap().value, 6);

        {
            let mut borrowed = example.try_borrow_mut().unwrap();
            borrowed.named_value_assign("fallback", &4.into()).unwrap();
            assert!(borrowed.named_value_assign("missing", &0.into()).is_err());
        }
        assert_eq!(example.cast::<Example>().unwrap().value, 12);
    });
}

#[test]
fn plugin_koto_impl_supports_object_arguments() {
    with_test_host_api(|| {
        let instance = KObject::from(Example { value: 7 });
        let other = KObject::from(Example { value: 7 });
        let args = [KValue::from(other)];
        let method = instance
            .try_borrow()
            .unwrap()
            .named_value("same_value")
            .unwrap()
            .unwrap();
        let mut vm = KotoVm::from_api(&TEST_HOST_API);

        assert!(matches!(
            vm.call_instance_function(instance.clone().into(), method, &args),
            Ok(KValue::Bool(true))
        ));
    });
}

#[test]
fn plugin_koto_impl_supports_mutating_self_with_object_arguments() {
    with_test_host_api(|| {
        let instance = KObject::from(Example { value: 0 });
        let other = KObject::from(Example { value: 123 });
        let args = [KValue::from(other)];
        let method = instance
            .try_borrow()
            .unwrap()
            .named_value("copy_value_from")
            .unwrap()
            .unwrap();
        let mut vm = KotoVm::from_api(&TEST_HOST_API);
        let result = vm
            .call_instance_function(instance.clone().into(), method, &args)
            .unwrap();
        assert!(matches!(result, KValue::Object(_)));
        assert_eq!(instance.cast::<Example>().unwrap().value, 123);
    });
}

#[test]
fn plugin_koto_fn_supports_plain_values() {
    with_test_host_api(|| {
        with_test_call_context(KValue::Null, vec![1.into(), 2.into()], |ctx| {
            assert!(matches!(add_numbers(ctx), Ok(KValue::Number(n)) if i64::from(n) == 3));
        });
    });
}

fn assert_number(value: Option<KValue>, expected: i64) {
    match value {
        Some(KValue::Number(number)) => assert_eq!(i64::from(number), expected),
        other => panic!("expected Some(Number({expected})), found {other:?}"),
    }
}

fn assert_string(value: Option<KValue>, expected: &str) {
    match value {
        Some(KValue::Str(string)) => assert_eq!(string, expected),
        other => panic!("expected Some(Str({expected:?})), found {other:?}"),
    }
}

#[test]
fn plugin_koto_fn_supports_object_arguments() {
    with_test_host_api(|| {
        with_test_call_context(
            KValue::Null,
            vec![
                KObject::from(Example { value: 42 }).into(),
                KObject::from(Example { value: 42 }).into(),
            ],
            |ctx| {
                assert!(matches!(examples_match(ctx), Ok(KValue::Bool(true))));
            },
        );
    });
}

#[test]
fn plugin_koto_fn_supports_vm_arguments() {
    with_test_host_api(|| {
        with_test_call_context(KValue::Null, vec![41.into()], |ctx| {
            assert!(matches!(add_with_vm(ctx), Ok(KValue::Number(n)) if i64::from(n) == 42));
        });
    });
}

#[test]
fn plugin_std_iterators_are_runtime_owned_objects() {
    with_test_host_api(|| {
        let iterator = KIterator::with_std_iter([1, 2].into_iter().map(KIteratorOutput::from));
        let mut vm = KotoVm::from_api(&TEST_HOST_API);

        assert!(
            matches!(vm.run_unary_op(UnaryOp::Next, iterator.clone().into()), Ok(KValue::Number(n)) if i64::from(n) == 1)
        );
        assert!(
            matches!(vm.run_unary_op(UnaryOp::NextBack, iterator.clone().into()), Ok(KValue::Number(n)) if i64::from(n) == 2)
        );
        assert!(matches!(
            vm.run_unary_op(UnaryOp::Next, iterator.into()),
            Ok(KValue::Null)
        ));
    });
}

#[test]
fn runtime_error_status_is_preserved_by_plugin_error_roundtrip() {
    RUNTIME_ERROR_CLONES.store(0, Ordering::SeqCst);
    RUNTIME_ERROR_FREES.store(0, Ordering::SeqCst);

    let error = crate::host::status_to_error(abi::KotoStatus {
        code: abi::KotoStatusCode::Error,
        error: Box::into_raw(Box::new(42usize)).cast(),
        clone_error: clone_test_runtime_error as *const c_void,
        free_error: free_test_runtime_error as *const c_void,
        is_unimplemented: true,
        message: std::ptr::null_mut(),
    });

    assert!(error.is_unimplemented_error());

    let cloned = error.clone();
    let status = error.into_status();
    let cloned_status = cloned.into_status();

    assert_eq!(RUNTIME_ERROR_CLONES.load(Ordering::SeqCst), 1);
    assert!(!status.error.is_null());
    assert!(!cloned_status.error.is_null());
    assert!(status.message.is_null());
    assert!(cloned_status.message.is_null());
    assert!(status.is_unimplemented);
    assert!(cloned_status.is_unimplemented);

    unsafe {
        (status.free_error_fn().unwrap())(status.error);
        (cloned_status.free_error_fn().unwrap())(cloned_status.error);
    }

    assert_eq!(RUNTIME_ERROR_FREES.load(Ordering::SeqCst), 2);
}
