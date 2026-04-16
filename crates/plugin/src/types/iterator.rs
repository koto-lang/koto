use crate::{
    Error, KObject, KValue, KotoVm,
    api::{
        KotoAccess, KotoBackend, KotoCopy, KotoObjectOps, KotoStaticType, KotoType, KotoVmTrait,
        UnaryOp,
    },
    host::current_host_api,
};
use koto_api::KotoIteratorBuilder;
use koto_ffi as abi;
use std::mem::ManuallyDrop;

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

impl<T> From<T> for KIteratorOutput
where
    KValue: From<T>,
{
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}

impl TryFrom<KIteratorOutput> for KValue {
    type Error = Error;

    fn try_from(output: KIteratorOutput) -> std::result::Result<Self, Self::Error> {
        match output {
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
    api: *const abi::KotoHostApiV1,
    handle: abi::OpaqueHandle,
}

impl KIterator {
    pub(crate) fn from_existing(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        let handle = unsafe { (api.value_clone)(handle) };
        Self::from_raw(api, handle)
    }

    fn from_raw(api: &abi::KotoHostApiV1, handle: abi::KValue) -> Self {
        debug_assert!(matches!(handle.kind, abi::KValueKind::Iterator));
        Self {
            api: api as *const _,
            handle: unsafe { handle.data.iterator_value },
        }
    }

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

    fn from_std_iterator_object(object: KObject) -> Self {
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

impl KotoIteratorBuilder for KIterator {
    type Item = KIteratorOutput;

    fn with_std_iter<T>(iter: T) -> Self
    where
        T: DoubleEndedIterator<Item = Self::Item> + Clone + Send + Sync + 'static,
    {
        Self::from_std_iterator_object(KObject::from(StdDoubleEndedIteratorObject { iter }))
    }

    fn with_std_forward_iter<T>(iter: T) -> Self
    where
        T: Iterator<Item = Self::Item> + Clone + Send + Sync + 'static,
    {
        Self::from_std_iterator_object(KObject::from(StdForwardIteratorObject { iter }))
    }
}

impl Clone for KIterator {
    fn clone(&self) -> Self {
        let api = self.api();
        Self::from_raw(api, unsafe { (api.value_clone)(self.handle()) })
    }
}

impl Drop for KIterator {
    fn drop(&mut self) {
        unsafe { (self.api().value_free)(self.handle()) };
    }
}

#[derive(Clone)]
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
