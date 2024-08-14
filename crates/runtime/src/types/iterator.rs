use crate::{prelude::*, vm::ReturnOrYield, Error, PtrMut, Result};
use std::{fmt, ops::DerefMut, result::Result as StdResult};

/// The trait used to implement iterators in Koto
///
/// See [KIterator].
pub trait KotoIterator: Iterator<Item = KIteratorOutput> + KotoSend + KotoSync {
    /// Returns a copy of the iterator that (when possible), will produce the same output
    fn make_copy(&self) -> Result<KIterator>;

    /// Returns true if the iterator supports reversed iteration via `next_back`
    fn is_bidirectional(&self) -> bool {
        false
    }

    /// Returns the next item produced by iterating backwards
    ///
    /// Returns `None` when no more items are available in reverse order.
    fn next_back(&mut self) -> Option<KIteratorOutput> {
        None
    }
}

/// The output type for iterators in Koto
#[derive(Clone)]
pub enum KIteratorOutput {
    /// A single value
    Value(KValue),
    /// A pair of values
    ///
    /// This is used as an optimization for iterators that output pairs, like a map iterator that
    /// outputs key/value pairs, or `enumerate`.
    ValuePair(KValue, KValue),
    /// An error that occurred during iteration
    ///
    /// Iterators that run functions should check for errors and pass them along to the caller.
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

    fn try_from(iterator_output: KIteratorOutput) -> StdResult<Self, Self::Error> {
        match iterator_output {
            KIteratorOutput::Value(value) => Ok(value),
            KIteratorOutput::ValuePair(first, second) => {
                Ok(KValue::Tuple(vec![first, second].into()))
            }
            KIteratorOutput::Error(error) => Err(error),
        }
    }
}

/// The iterator value type used in Koto
#[derive(Clone)]
pub struct KIterator(PtrMut<dyn KotoIterator>);

impl KIterator {
    /// Creates a new KIterator from any value that implements [KotoIterator]
    pub fn new(external: impl KotoIterator + 'static) -> Self {
        Self(make_ptr_mut!(external))
    }

    /// Creates a new KIterator from any iterator that implements DoubleEndedIterator
    ///
    /// This should only be used for iterators without side-effects.
    pub fn with_std_iter<T>(iter: T) -> Self
    where
        T: DoubleEndedIterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
    {
        Self::new(StdDoubleEndedIterator::<T> { iter })
    }

    /// Creates a new KIterator from any iterator that implements Iterator
    ///
    /// This should only be used for iterators without side-effects.
    pub fn with_std_forward_iter<T>(iter: T) -> Self
    where
        T: Iterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
    {
        Self::new(StdForwardIterator::<T> { iter })
    }

    /// Creates a new KIterator from a Range
    pub fn with_range(range: KRange) -> Result<Self> {
        Ok(Self::new(RangeIterator::new(range)?))
    }

    /// Creates a new KIterator from a List
    pub fn with_list(list: KList) -> Self {
        Self::new(ListIterator::new(list))
    }

    /// Creates a new KIterator from a Tuple
    pub fn with_tuple(tuple: KTuple) -> Self {
        Self::new(TupleIterator::new(tuple))
    }

    /// Creates a new KIterator from a Map
    pub fn with_map(map: KMap) -> Self {
        Self::new(MapIterator::new(map))
    }

    /// Creates a new KIterator from a String
    pub fn with_string(s: KString) -> Self {
        Self::new(StringIterator::new(s))
    }

    /// Creates a new KIterator from a Vm, used to implement generators
    pub fn with_vm(vm: KotoVm) -> Self {
        Self::new(GeneratorIterator::new(vm))
    }

    /// Creates a new KIterator from a Value that has an implementation of `@next`
    pub fn with_meta_next(vm: KotoVm, iterator: KValue) -> Result<Self> {
        Ok(Self::new(MetaIterator::new(vm, iterator)?))
    }

    /// Creates a new KIterator from an Object that implements [KotoIterator]
    pub fn with_object(vm: KotoVm, o: KObject) -> Result<Self> {
        Ok(Self::new(ObjectIterator::new(vm, o)?))
    }

    /// Creates a new KIterator that yields a value once
    pub fn once(value: KValue) -> Result<Self> {
        Ok(Self::new(crate::core_lib::iterator::generators::Once::new(
            value,
        )))
    }

    /// Makes a copy of the iterator
    ///
    /// See [KotoIterator::make_copy]
    pub fn make_copy(&self) -> Result<Self> {
        self.0.borrow().make_copy()
    }

    /// Returns true if the iterator supports reversed iteration via `next_back`
    ///
    /// See [KotoIterator::is_bidirectional]
    pub fn is_bidirectional(&self) -> bool {
        self.0.borrow().is_bidirectional()
    }

    /// Returns the next item produced by iterating backwards
    ///
    /// See [KotoIterator::next_back]
    pub fn next_back(&mut self) -> Option<KIteratorOutput> {
        self.0.borrow_mut().next_back()
    }

    /// Mutably borrows the underlying iterator, allowing repeated iterations with a single borrow
    pub fn borrow_internals(
        &mut self,
        mut f: impl FnMut(&mut dyn KotoIterator) -> Option<KIteratorOutput>,
    ) -> Option<KIteratorOutput> {
        f(self.0.borrow_mut().deref_mut())
    }
}

impl Iterator for KIterator {
    type Item = KIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.borrow_mut().next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.borrow().size_hint()
    }
}

impl fmt::Debug for KIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KIterator")
    }
}

// Convenience type alias for the rest of this module
type Output = KIteratorOutput;

#[derive(Clone)]
struct RangeIterator {
    range: KRange,
}

impl RangeIterator {
    fn new(range: KRange) -> Result<Self> {
        if range.is_bounded() {
            Ok(Self { range })
        } else {
            runtime_error!("Unbounded ranges can't be used as iterators (range: {range})")
        }
    }
}

impl KotoIterator for RangeIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        match self.range.pop_back() {
            Ok(Some(output)) => Some(KIteratorOutput::Value(output.into())),
            Ok(None) => None,
            Err(e) => Some(KIteratorOutput::Error(e)),
        }
    }
}

impl Iterator for RangeIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.range.pop_front() {
            Ok(Some(output)) => Some(KIteratorOutput::Value(output.into())),
            Ok(None) => None,
            Err(e) => Some(KIteratorOutput::Error(e)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Unwrap: only bounded ranges can be used as iterators
        let remaining = self.range.size().unwrap();
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct ListIterator {
    data: KList,
    index: usize,
    end: usize,
}

impl ListIterator {
    fn new(data: KList) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn get_output(&self, index: usize) -> Option<KIteratorOutput> {
        self.data
            .data()
            .get(index)
            .map(|data| KIteratorOutput::Value(data.clone()))
    }
}

impl KotoIterator for ListIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        if self.end > self.index {
            self.end -= 1;
            self.get_output(self.end)
        } else {
            None
        }
    }
}

impl Iterator for ListIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end > self.index {
            let result = self.get_output(self.index);
            self.index += 1;
            result
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct TupleIterator(KTuple);

impl TupleIterator {
    fn new(tuple: KTuple) -> Self {
        Self(tuple)
    }
}

impl KotoIterator for TupleIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        self.0.pop_back().map(KIteratorOutput::Value)
    }
}

impl Iterator for TupleIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front().map(KIteratorOutput::Value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.0.len();
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct MapIterator {
    data: KMap,
    index: usize,
    end: usize,
}

impl MapIterator {
    fn new(data: KMap) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn get_output(&self, index: usize) -> Option<KIteratorOutput> {
        self.data
            .data()
            .get_index(index)
            .map(|(key, value)| KIteratorOutput::ValuePair(key.value().clone(), value.clone()))
    }
}

impl KotoIterator for MapIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        if self.end > self.index {
            self.end -= 1;
            self.get_output(self.end)
        } else {
            None
        }
    }
}

impl Iterator for MapIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end > self.index {
            let result = self.get_output(self.index);
            self.index += 1;
            result
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.data().len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct MetaIterator {
    vm: KotoVm,
    iterator: KValue,
    is_bidirectional: bool,
}

impl MetaIterator {
    fn new(vm: KotoVm, iterator: KValue) -> Result<Self> {
        let KValue::Map(m) = &iterator else {
            return runtime_error!("Expected Map with implementation of @next");
        };

        match m.get_meta_value(&UnaryOp::Next.into()) {
            Some(op) if op.is_callable() => {}
            Some(op) => return unexpected_type("Callable function from @next", &op),
            None => return runtime_error!("Expected implementation of @next"),
        };

        let is_bidirectional = match m.get_meta_value(&UnaryOp::NextBack.into()) {
            Some(op) if op.is_callable() => true,
            Some(op) => return unexpected_type("Callable function from @next_back", &op),
            None => false,
        };

        Ok(Self {
            vm,
            iterator,
            is_bidirectional,
        })
    }
}

impl KotoIterator for MetaIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        self.is_bidirectional
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        match self
            .vm
            .run_unary_op(UnaryOp::NextBack, self.iterator.clone())
        {
            Ok(KValue::Null) => None,
            Ok(result) => Some(Output::Value(result)),
            Err(error) => Some(Output::Error(error)),
        }
    }
}

impl Iterator for MetaIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.vm.run_unary_op(UnaryOp::Next, self.iterator.clone()) {
            Ok(KValue::Null) => None,
            Ok(result) => Some(Output::Value(result)),
            Err(error) => Some(Output::Error(error)),
        }
    }
}

#[derive(Clone)]
struct ObjectIterator {
    vm: KotoVm,
    object: KObject,
}

impl ObjectIterator {
    fn new(vm: KotoVm, object: KObject) -> Result<Self> {
        use IsIterable::*;

        if matches!(
            object.try_borrow()?.is_iterable(),
            ForwardIterator | BidirectionalIterator
        ) {
            Ok(Self { vm, object })
        } else {
            runtime_error!("{} is not an iterator", object.try_borrow()?.type_string())
        }
    }
}

impl KotoIterator for ObjectIterator {
    fn make_copy(&self) -> Result<KIterator> {
        let copy = Self {
            vm: self.vm.spawn_shared_vm(),
            object: self.object.try_borrow()?.copy(),
        };
        Ok(KIterator::new(copy))
    }

    fn is_bidirectional(&self) -> bool {
        self.object.try_borrow().map_or(false, |o| {
            matches!(o.is_iterable(), IsIterable::BidirectionalIterator)
        })
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        match self.object.try_borrow_mut() {
            Ok(mut o) => o.iterator_next_back(&mut self.vm),
            Err(e) => Some(KIteratorOutput::Error(e)),
        }
    }
}

impl Iterator for ObjectIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.object.try_borrow_mut() {
            Ok(mut o) => o.iterator_next(&mut self.vm),
            Err(e) => Some(KIteratorOutput::Error(e)),
        }
    }
}

/// An iterator that yields the characters contained in the string
#[derive(Clone)]
pub struct StringIterator(KString);

impl StringIterator {
    pub fn new(s: KString) -> Self {
        Self(s)
    }
}

impl KotoIterator for StringIterator {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        self.0.pop_back().map(|s| KIteratorOutput::Value(s.into()))
    }
}

impl Iterator for StringIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front().map(|s| KIteratorOutput::Value(s.into()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.0.len();
        let lower_bound = (upper_bound != 0) as usize;
        (lower_bound, Some(upper_bound))
    }
}

#[derive(Clone)]
pub struct GeneratorIterator {
    vm: KotoVm,
}

impl GeneratorIterator {
    pub fn new(vm: KotoVm) -> Self {
        Self { vm }
    }
}

impl KotoIterator for GeneratorIterator {
    fn make_copy(&self) -> Result<KIterator> {
        let new_vm = crate::vm::clone_generator_vm(&self.vm)?;
        Ok(KIterator::with_vm(new_vm))
    }
}

impl Iterator for GeneratorIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.vm.continue_running() {
            Ok(ReturnOrYield::Return(_)) => None,
            Ok(ReturnOrYield::Yield(output)) => match output {
                KValue::TemporaryTuple(_) => {
                    unreachable!("Yield shouldn't produce temporary tuples")
                }
                result => Some(KIteratorOutput::Value(result)),
            },
            Err(error) => Some(KIteratorOutput::Error(error)),
        }
    }
}

#[derive(Clone)]
pub struct StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    iter: T,
}

impl<T> KotoIterator for StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl<T> Iterator for StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[derive(Clone)]
pub struct StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    iter: T,
}

impl<T> KotoIterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<KIteratorOutput> {
        self.iter.next_back()
    }
}

impl<T> Iterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<T> DoubleEndedIterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + KotoSend + KotoSync + 'static,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}
