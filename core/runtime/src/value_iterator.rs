use {
    crate::prelude::*,
    std::{cell::RefCell, cmp::Ordering, fmt, ops::DerefMut, rc::Rc},
};

/// The trait used to implement iterators in Koto
///
/// See `ValueIterator`.
pub trait KotoIterator: Iterator<Item = ValueIteratorOutput> {
    /// Returns a copy of the iterator that (when possible), will produce the same output
    fn make_copy(&self) -> ValueIterator;

    /// Returns true if the iterator supports reversed iteration via `next_back`
    fn is_bidirectional(&self) -> bool {
        false
    }

    /// Returns the next item produced by iterating backwards
    ///
    /// Returns `None` when no more items are available in reverse order.
    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        None
    }
}

/// The output type for iterators in Koto
#[derive(Clone, Debug)]
pub enum ValueIteratorOutput {
    /// A single value
    Value(Value),
    /// A pair of values
    ///
    /// This is used as an optimization for iterators that output pairs, like a map iterator that
    /// outputs key/value pairs, or `enumerate`.
    ValuePair(Value, Value),
    /// An error that occurred during iteration
    ///
    /// Iterators that run functions should check for errors and pass them along to the caller.
    Error(RuntimeError),
}

impl<T> From<T> for ValueIteratorOutput
where
    Value: From<T>,
{
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}

/// The iterator value type used in Koto
#[derive(Clone)]
pub struct ValueIterator(PtrMut<dyn KotoIterator>);

impl ValueIterator {
    /// Creates a new ValueIterator from any value that implements [KotoIterator]
    pub fn new(external: impl KotoIterator + 'static) -> Self {
        Self(PtrMut::from(
            Rc::new(RefCell::new(external)) as Rc<RefCell<dyn KotoIterator>>
        ))
    }

    /// Creates a new ValueIterator from any iterator that implements DoubleEndedIterator
    ///
    /// This should only be used for iterators without side-effects.
    pub fn with_std_iter<T>(iter: T) -> Self
    where
        T: DoubleEndedIterator<Item = Output> + Clone + 'static,
    {
        Self::new(StdDoubleEndedIterator::<T> { iter })
    }

    /// Creates a new ValueIterator from any iterator that implements Iterator
    ///
    /// This should only be used for iterators without side-effects.
    pub fn with_std_forward_iter<T>(iter: T) -> Self
    where
        T: Iterator<Item = Output> + Clone + 'static,
    {
        Self::new(StdForwardIterator::<T> { iter })
    }

    /// Creates a new ValueIterator from a Range
    pub fn with_range(range: IntRange) -> Result<Self, RuntimeError> {
        Ok(Self::new(RangeIterator::new(range)?))
    }

    /// Creates a new ValueIterator from a List
    pub fn with_list(list: ValueList) -> Self {
        Self::new(ListIterator::new(list))
    }

    /// Creates a new ValueIterator from a Tuple
    pub fn with_tuple(tuple: ValueTuple) -> Self {
        Self::new(TupleIterator::new(tuple))
    }

    /// Creates a new ValueIterator from a Map
    pub fn with_map(map: ValueMap) -> Self {
        Self::new(MapIterator::new(map))
    }

    /// Creates a new ValueIterator from a String
    pub fn with_string(s: ValueString) -> Self {
        Self::new(StringIterator::new(s))
    }

    /// Creates a new ValueIterator from a Vm, used to implement generators
    pub fn with_vm(vm: Vm) -> Self {
        Self::new(GeneratorIterator::new(vm))
    }

    /// Makes a copy of the iterator
    ///
    /// See [KotoIterator::make_copy]
    #[must_use]
    pub fn make_copy(&self) -> Self {
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
    pub fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        self.0.borrow_mut().next_back()
    }

    /// Mutably borrows the underlying iterator, allowing repeated iterations with a single borrow
    pub fn borrow_internals(
        &mut self,
        mut f: impl FnMut(&mut dyn KotoIterator) -> Option<ValueIteratorOutput>,
    ) -> Option<ValueIteratorOutput> {
        f(self.0.borrow_mut().deref_mut())
    }
}

impl Iterator for ValueIterator {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.borrow_mut().next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.borrow().size_hint()
    }
}

impl fmt::Debug for ValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ValueIterator")
    }
}

// Convenience type alias for the rest of this module
type Output = ValueIteratorOutput;

#[derive(Clone)]
struct RangeIterator {
    start: isize,
    end: isize,
}

impl RangeIterator {
    fn new(range: IntRange) -> Result<Self, RuntimeError> {
        use Ordering::*;

        match (range.start, range.end) {
            (Some(start), Some((end, inclusive))) => {
                let end = match start.cmp(&end) {
                    Less => {
                        if inclusive {
                            end + 1
                        } else {
                            end
                        }
                    }
                    Greater => {
                        if inclusive {
                            end - 1
                        } else {
                            end
                        }
                    }
                    Equal => end,
                };
                Ok(Self { start, end })
            }
            _ => runtime_error!("Unbounded ranges can't be used as iterators (range: {range})"),
        }
    }
}

impl KotoIterator for RangeIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        match self.start.cmp(&self.end) {
            Ordering::Less => self.end -= 1,
            Ordering::Greater => self.end += 1,
            Ordering::Equal => return None,
        }

        Some(ValueIteratorOutput::Value(Value::Number(self.end.into())))
    }
}

impl Iterator for RangeIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.start.cmp(&self.end) {
            Ordering::Less => {
                let result = self.start;
                self.start += 1;
                result
            }
            Ordering::Greater => {
                let result = self.start;
                self.start -= 1;
                result
            }
            Ordering::Equal => return None,
        };

        Some(ValueIteratorOutput::Value(Value::Number(result.into())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end - self.start).unsigned_abs();
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct ListIterator {
    data: ValueList,
    index: usize,
    end: usize,
}

impl ListIterator {
    fn new(data: ValueList) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn get_output(&self, index: usize) -> Option<ValueIteratorOutput> {
        self.data
            .data()
            .get(index)
            .map(|data| ValueIteratorOutput::Value(data.clone()))
    }
}

impl KotoIterator for ListIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
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
struct TupleIterator(ValueTuple);

impl TupleIterator {
    fn new(tuple: ValueTuple) -> Self {
        Self(tuple)
    }
}

impl KotoIterator for TupleIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        self.0.pop_back().map(ValueIteratorOutput::Value)
    }
}

impl Iterator for TupleIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front().map(ValueIteratorOutput::Value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.0.len();
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct MapIterator {
    data: ValueMap,
    index: usize,
    end: usize,
}

impl MapIterator {
    fn new(data: ValueMap) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn get_output(&self, index: usize) -> Option<ValueIteratorOutput> {
        self.data
            .data()
            .get_index(index)
            .map(|(key, value)| ValueIteratorOutput::ValuePair(key.value().clone(), value.clone()))
    }
}

impl KotoIterator for MapIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
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

/// An iterator that yields the characters contained in the string
#[derive(Clone)]
pub struct StringIterator(ValueString);

impl StringIterator {
    pub fn new(s: ValueString) -> Self {
        Self(s)
    }
}

impl KotoIterator for StringIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        self.0
            .pop_back()
            .map(|s| ValueIteratorOutput::Value(s.into()))
    }
}

impl Iterator for StringIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .pop_front()
            .map(|s| ValueIteratorOutput::Value(s.into()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.0.len();
        let lower_bound = (upper_bound != 0) as usize;
        (lower_bound, Some(upper_bound))
    }
}

#[derive(Clone)]
pub struct GeneratorIterator {
    vm: Vm,
}

impl GeneratorIterator {
    pub fn new(vm: Vm) -> Self {
        Self { vm }
    }
}

impl KotoIterator for GeneratorIterator {
    fn make_copy(&self) -> ValueIterator {
        let new_vm = crate::vm::clone_generator_vm(&self.vm);
        ValueIterator::with_vm(new_vm)
    }
}

impl Iterator for GeneratorIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.vm.continue_running() {
            Ok(Value::Null) => None,
            Ok(Value::TemporaryTuple(_)) => {
                unreachable!("Yield shouldn't produce temporary tuples")
            }
            Ok(result) => Some(ValueIteratorOutput::Value(result)),
            Err(error) => Some(ValueIteratorOutput::Error(error)),
        }
    }
}

#[derive(Clone)]
pub struct StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + 'static,
{
    iter: T,
}

impl<T> KotoIterator for StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + 'static,
{
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }
}

impl<T> Iterator for StdForwardIterator<T>
where
    T: Iterator<Item = Output> + Clone + 'static,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[derive(Clone)]
pub struct StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + 'static,
{
    iter: T,
}

impl<T> KotoIterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + 'static,
{
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        self.iter.next_back()
    }
}

impl<T> Iterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + 'static,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<T> DoubleEndedIterator for StdDoubleEndedIterator<T>
where
    T: DoubleEndedIterator<Item = Output> + Clone + 'static,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}
