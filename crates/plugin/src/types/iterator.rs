use crate::abi;
use crate::{
    Error, KObject, KValue, KotoVm,
    api::{KotoAccess, KotoBackend, KotoCopy, KotoObjectOps, KotoStaticType, KotoType},
};
use koto_api::KotoIteratorBuilder;
use std::mem::ManuallyDrop;
cfg_select! {
    target_arch = "wasm32" => {
        use crate::wasm_support;
        use koto_ffi::wasm;
        use std::{
            cell::{Cell, RefCell},
            collections::HashMap,
        };
    }
    _ => {
        use crate::host::current_host_api;
        use crate::api::{KotoVmTrait, UnaryOp};
    }
}

/// The output type for iterators in `koto_plugin`.
#[derive(Clone)]
pub enum KIteratorOutput {
    /// A single value.
    Value(KValue),
    /// A pair of values.
    ValuePair(KValue, KValue),
    /// An error that occurred during iteration.
    Error(Error),
}

macro_rules! impl_iterator_output_from {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for KIteratorOutput {
                fn from(value: $ty) -> Self {
                    Self::Value(value.into())
                }
            }
        )*
    };
}

impl_iterator_output_from!(
    bool,
    i64,
    i32,
    usize,
    f32,
    f64,
    String,
    &str,
    KValue,
    crate::KNumber,
    crate::KRange,
    crate::KList,
    crate::KTuple,
    crate::KMap,
    crate::KFunction,
    crate::KNativeFunction,
    crate::KIterator,
    crate::KObject,
);

impl From<(KValue, KValue)> for KIteratorOutput {
    fn from((first, second): (KValue, KValue)) -> Self {
        Self::ValuePair(first, second)
    }
}

impl From<Error> for KIteratorOutput {
    fn from(error: Error) -> Self {
        Self::Error(error)
    }
}

impl KIteratorOutput {
    /// Converts iterator output into a plain value, turning iterator errors into `Err`.
    pub fn try_into_value(self) -> std::result::Result<KValue, Error> {
        match self {
            KIteratorOutput::Value(value) => Ok(value),
            KIteratorOutput::ValuePair(first, second) => {
                Ok(KValue::Tuple(vec![first, second].into()))
            }
            KIteratorOutput::Error(error) => Err(error),
        }
    }
}

/// The iterator value type used in `koto_plugin`.
#[derive(Debug)]
pub struct KIterator {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    api: *const abi::KotoHostApiV1,
    handle: abi::OpaqueHandle,
}

#[cfg(target_arch = "wasm32")]
trait WasmIteratorState {
    fn make_copy(&self) -> Box<dyn WasmIteratorState>;

    fn is_bidirectional(&self) -> bool {
        false
    }

    fn next(&mut self) -> Option<KIteratorOutput>;

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        None
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    // Registered wasm iterators are removed again via `koto_plugin_iterator_drop_v1` when the
    // host releases the last handle or runtime wrapper referencing the guest iterator.
    static WASM_ITERATORS: RefCell<HashMap<u32, Box<dyn WasmIteratorState>>> = RefCell::new(HashMap::new());
    static NEXT_WASM_ITERATOR_ID: Cell<u32> = const { Cell::new(1) };
}

#[cfg(target_arch = "wasm32")]
fn register_wasm_iterator(iterator: Box<dyn WasmIteratorState>) -> u32 {
    NEXT_WASM_ITERATOR_ID.with(|next_id| {
        let id = next_id.get();
        next_id.set(id + 1);
        WASM_ITERATORS.with(|iterators| {
            iterators.borrow_mut().insert(id, iterator);
        });
        id
    })
}

#[cfg(target_arch = "wasm32")]
fn unregister_wasm_iterator(id: u32) {
    WASM_ITERATORS.with(|iterators| {
        iterators.borrow_mut().remove(&id);
    });
}

#[cfg(target_arch = "wasm32")]
fn with_wasm_iterator_mut<T>(
    id: u32,
    f: impl FnOnce(&mut dyn WasmIteratorState) -> T,
) -> Option<T> {
    WASM_ITERATORS.with(|iterators| {
        let mut iterators = iterators.borrow_mut();
        iterators.get_mut(&id).map(|iterator| f(iterator.as_mut()))
    })
}

#[cfg(target_arch = "wasm32")]
fn with_wasm_iterator<T>(id: u32, f: impl FnOnce(&dyn WasmIteratorState) -> T) -> Option<T> {
    WASM_ITERATORS.with(|iterators| {
        let iterators = iterators.borrow();
        iterators.get(&id).map(|iterator| f(iterator.as_ref()))
    })
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct WasmForwardIterator<T> {
    iter: T,
}

#[cfg(target_arch = "wasm32")]
impl<T> WasmIteratorState for WasmForwardIterator<T>
where
    T: Iterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn make_copy(&self) -> Box<dyn WasmIteratorState> {
        Box::new(self.clone())
    }

    fn next(&mut self) -> Option<KIteratorOutput> {
        self.iter.next()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct WasmDoubleEndedIterator<T> {
    iter: T,
}

#[cfg(target_arch = "wasm32")]
impl<T> WasmIteratorState for WasmDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn make_copy(&self) -> Box<dyn WasmIteratorState> {
        Box::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next(&mut self) -> Option<KIteratorOutput> {
        self.iter.next()
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        self.iter.next_back()
    }
}

impl KIterator {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_wasm_existing(handle: wasm::KValue) -> Self {
        let handle = unsafe { wasm::value_clone(handle) };
        let handle = wasm_support::wasm_value_to_native(handle);
        debug_assert!(matches!(handle.kind, abi::KValueKind::Iterator));
        Self {
            api: std::ptr::null(),
            handle: unsafe { handle.data.iterator_value },
        }
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::Iterator));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.iterator_value },
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn api(&self) -> &abi::KotoHostApiV1 {
        unsafe { &*self.api }
    }

    fn handle(&self) -> abi::KValue {
        abi::KValue {
            kind: abi::KValueKind::Iterator,
            data: abi::KValueData {
                iterator_value: self.handle,
            },
        }
    }

    pub(crate) fn into_raw(self) -> abi::KValue {
        let this = ManuallyDrop::new(self);
        abi::KValue {
            kind: abi::KValueKind::Iterator,
            data: abi::KValueData {
                iterator_value: this.handle,
            },
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn from_std_iterator_object(object: KObject) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = object;
                panic!("std iterator object conversion isn't implemented for wasm");
            }
            _ => {
                let api = current_host_api();
                let mut vm = KotoVm::from_api(api);

                match KotoVmTrait::run_unary_op(&mut vm, UnaryOp::Iterator, object.into()) {
                    Ok(KValue::Iterator(iterator)) => iterator,
                    Ok(unexpected) => panic!(
                        "expected Iterator from @iterator, found {}",
                        unexpected.type_as_string()
                    ),
                    Err(error) => panic!("failed to create iterator from std iterator: {error:?}"),
                }
            }
        }
    }
}

impl KotoIteratorBuilder for KIterator {
    type Item = KIteratorOutput;

    fn with_std_iter<T>(iter: T) -> Self
    where
        T: DoubleEndedIterator<Item = Self::Item> + Clone + Send + Sync + 'static,
    {
        cfg_select! {
            target_arch = "wasm32" => {
                let id = register_wasm_iterator(Box::new(WasmDoubleEndedIterator { iter }));
                let handle = wasm_support::wasm_value_to_native(unsafe { wasm::iterator_make(id) });
                debug_assert!(matches!(handle.kind, abi::KValueKind::Iterator));
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { handle.data.iterator_value },
                }
            }
            _ => {
                Self::from_std_iterator_object(KObject::from(StdDoubleEndedIteratorObject { iter }))
            }
        }
    }

    fn with_std_forward_iter<T>(iter: T) -> Self
    where
        T: Iterator<Item = Self::Item> + Clone + Send + Sync + 'static,
    {
        cfg_select! {
            target_arch = "wasm32" => {
                let id = register_wasm_iterator(Box::new(WasmForwardIterator { iter }));
                let handle = wasm_support::wasm_value_to_native(unsafe { wasm::iterator_make(id) });
                debug_assert!(matches!(handle.kind, abi::KValueKind::Iterator));
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { handle.data.iterator_value },
                }
            }
            _ => {
                Self::from_std_iterator_object(KObject::from(StdForwardIteratorObject { iter }))
            }
        }
    }
}

impl Clone for KIterator {
    fn clone(&self) -> Self {
        cfg_select! {
            target_arch = "wasm32" => {
                let cloned = unsafe {
                    wasm::value_clone(wasm_support::native_value_to_wasm(self.handle()))
                };
                let cloned = wasm_support::wasm_value_to_native(cloned);
                Self {
                    api: std::ptr::null(),
                    handle: unsafe { cloned.data.iterator_value },
                }
            }
            _ => {
                let api = self.api();
                Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
            }
        }
    }
}

impl Drop for KIterator {
    fn drop(&mut self) {
        cfg_select! {
            target_arch = "wasm32" => unsafe {
                wasm::value_free(wasm_support::native_value_to_wasm(self.handle()));
            },
            _ => unsafe {
                (self.api().value_free)(self.handle())
            },
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_iterator_is_bidirectional_v1")]
unsafe extern "C" fn wasm_iterator_is_bidirectional_trampoline(
    user_data: u32,
    out_ptr: u32,
    status_ptr: u32,
) {
    let (status, out) = match with_wasm_iterator(user_data, |iterator| iterator.is_bidirectional())
    {
        Some(is_bidirectional) => (wasm::KotoStatus::ok(), is_bidirectional),
        None => (
            wasm_support::error_status("unknown wasm plugin iterator id"),
            false,
        ),
    };

    unsafe {
        *(out_ptr as *mut bool) = out;
        *(status_ptr as *mut wasm::KotoStatus) = status;
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_iterator_copy_v1")]
unsafe extern "C" fn wasm_iterator_copy_trampoline(user_data: u32, out_ptr: u32, status_ptr: u32) {
    let (status, out) = match with_wasm_iterator(user_data, |iterator| iterator.make_copy()) {
        Some(iterator) => {
            let id = register_wasm_iterator(iterator);
            (wasm::KotoStatus::ok(), unsafe { wasm::iterator_make(id) })
        }
        None => (
            wasm_support::error_status("unknown wasm plugin iterator id"),
            wasm::KValue::null(),
        ),
    };

    unsafe {
        *(out_ptr as *mut wasm::KValue) = out;
        *(status_ptr as *mut wasm::KotoStatus) = status;
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_iterator_next_v1")]
unsafe extern "C" fn wasm_iterator_next_trampoline(
    user_data: u32,
    out_ptr: u32,
    out_has_value_ptr: u32,
    status_ptr: u32,
) {
    let (status, out, has_value) =
        match with_wasm_iterator_mut(user_data, |iterator| iterator.next()) {
            Some(Some(output)) => match output.try_into_value() {
                Ok(value) => (
                    wasm::KotoStatus::ok(),
                    crate::types::map::encode_export_value(value),
                    true,
                ),
                Err(error) => (
                    wasm_support::error_status(&error.to_string()),
                    wasm::KValue::null(),
                    false,
                ),
            },
            Some(None) => (wasm::KotoStatus::ok(), wasm::KValue::null(), false),
            None => (
                wasm_support::error_status("unknown wasm plugin iterator id"),
                wasm::KValue::null(),
                false,
            ),
        };

    unsafe {
        *(out_ptr as *mut wasm::KValue) = out;
        *(out_has_value_ptr as *mut bool) = has_value;
        *(status_ptr as *mut wasm::KotoStatus) = status;
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_iterator_next_back_v1")]
unsafe extern "C" fn wasm_iterator_next_back_trampoline(
    user_data: u32,
    out_ptr: u32,
    out_has_value_ptr: u32,
    status_ptr: u32,
) {
    let (status, out, has_value) =
        match with_wasm_iterator_mut(user_data, |iterator| iterator.next_back()) {
            Some(Some(output)) => match output.try_into_value() {
                Ok(value) => (
                    wasm::KotoStatus::ok(),
                    crate::types::map::encode_export_value(value),
                    true,
                ),
                Err(error) => (
                    wasm_support::error_status(&error.to_string()),
                    wasm::KValue::null(),
                    false,
                ),
            },
            Some(None) => (wasm::KotoStatus::ok(), wasm::KValue::null(), false),
            None => (
                wasm_support::error_status("unknown wasm plugin iterator id"),
                wasm::KValue::null(),
                false,
            ),
        };

    unsafe {
        *(out_ptr as *mut wasm::KValue) = out;
        *(out_has_value_ptr as *mut bool) = has_value;
        *(status_ptr as *mut wasm::KotoStatus) = status;
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "koto_plugin_iterator_drop_v1")]
unsafe extern "C" fn wasm_iterator_drop_trampoline(user_data: u32) {
    unregister_wasm_iterator(user_data);
}

#[derive(Clone)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct StdForwardIteratorObject<T> {
    iter: T,
}

impl<T> KotoStaticType for StdForwardIteratorObject<T> {
    fn type_static() -> &'static str {
        "Iterator"
    }
}

impl<T, B: KotoBackend> KotoType<B> for StdForwardIteratorObject<T> {
    fn type_string(&self) -> B::String {
        Self::type_static().into()
    }
}

impl<T> KotoCopy<crate::Backend> for StdForwardIteratorObject<T>
where
    T: Iterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn copy(&self) -> KObject {
        self.clone().into()
    }
}

impl<T, B: KotoBackend> KotoAccess<B> for StdForwardIteratorObject<T> where
    T: Iterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static
{
}

impl<T> KotoObjectOps<crate::Backend> for StdForwardIteratorObject<T>
where
    T: Iterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn is_iterable(&self) -> crate::Result<crate::IsIterable> {
        Ok(crate::IsIterable::ForwardIterator)
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> crate::Result<Option<KIteratorOutput>> {
        Ok(self.iter.next())
    }
}

#[derive(Clone)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct StdDoubleEndedIteratorObject<T> {
    iter: T,
}

impl<T> KotoStaticType for StdDoubleEndedIteratorObject<T> {
    fn type_static() -> &'static str {
        "Iterator"
    }
}

impl<T, B: KotoBackend> KotoType<B> for StdDoubleEndedIteratorObject<T> {
    fn type_string(&self) -> B::String {
        Self::type_static().into()
    }
}

impl<T> KotoCopy<crate::Backend> for StdDoubleEndedIteratorObject<T>
where
    T: DoubleEndedIterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn copy(&self) -> KObject {
        self.clone().into()
    }
}

impl<T, B: KotoBackend> KotoAccess<B> for StdDoubleEndedIteratorObject<T> where
    T: DoubleEndedIterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static
{
}

impl<T> KotoObjectOps<crate::Backend> for StdDoubleEndedIteratorObject<T>
where
    T: DoubleEndedIterator<Item = KIteratorOutput> + Clone + Send + Sync + 'static,
{
    fn is_iterable(&self) -> crate::Result<crate::IsIterable> {
        Ok(crate::IsIterable::BidirectionalIterator)
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> crate::Result<Option<KIteratorOutput>> {
        Ok(self.iter.next())
    }

    fn iterator_next_back(&mut self, _vm: &mut KotoVm) -> crate::Result<Option<KIteratorOutput>> {
        Ok(self.iter.next_back())
    }
}
