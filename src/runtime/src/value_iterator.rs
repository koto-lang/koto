use {
    crate::{IntRange, RuntimeError, Value, ValueList, ValueMap, ValueString, ValueTuple, Vm},
    std::{cell::RefCell, cmp::Ordering, fmt, ops::DerefMut, rc::Rc},
    unicode_segmentation::GraphemeCursor,
};

/// The trait used to implement iterators in Koto
///
/// See `ValueIterator`.
pub trait KotoIterator: Iterator<Item = ValueIteratorOutput> {
    /// Returns a copy of the iterator that (when possible), will produce the same output
    fn make_copy(&self) -> ValueIterator;

    /// Returns true when the iterator executes functions that may cause side effects
    ///
    /// This is used to determine whether or not the iterator is repeatable, which is used in
    /// iterator adaptors like chunks() or windows().
    fn might_have_side_effects(&self) -> bool;

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
pub struct ValueIterator(Rc<RefCell<dyn KotoIterator>>);

impl ValueIterator {
    /// Creates a new ValueIterator from any value that implements [KotoIterator]
    pub fn new(external: impl KotoIterator + 'static) -> Self {
        Self(Rc::new(RefCell::new(external)))
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
    pub fn with_range(range: IntRange) -> Self {
        Self::new(RangeIterator::new(range))
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

    /// Returns true if the iterator might have side-effects
    ///
    /// See [KotoIterator::might_have_side_effects]
    pub fn might_have_side_effects(&self) -> bool {
        self.0.borrow().might_have_side_effects()
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
    range: IntRange,
}

impl RangeIterator {
    fn new(range: IntRange) -> Self {
        Self { range }
    }
}

impl KotoIterator for RangeIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        let range = &mut self.range;
        match range.start.cmp(&range.end) {
            Ordering::Less => range.end -= 1,
            Ordering::Greater => range.end += 1,
            Ordering::Equal => return None,
        }

        Some(ValueIteratorOutput::Value(Value::Number(range.end.into())))
    }
}

impl Iterator for RangeIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let range = &mut self.range;

        let result = match range.start.cmp(&range.end) {
            Ordering::Less => {
                let result = range.start;
                range.start += 1;
                result
            }
            Ordering::Greater => {
                let result = range.start;
                range.start -= 1;
                result
            }
            Ordering::Equal => return None,
        };

        Some(ValueIteratorOutput::Value(Value::Number(result.into())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.range.len();
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

    fn might_have_side_effects(&self) -> bool {
        false
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
struct TupleIterator {
    data: ValueTuple,
    index: usize,
    end: usize,
}

impl TupleIterator {
    fn new(data: ValueTuple) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn get_output(&self, index: usize) -> Option<ValueIteratorOutput> {
        self.data
            .get(index)
            .map(|data| ValueIteratorOutput::Value(data.clone()))
    }
}

impl KotoIterator for TupleIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
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

impl Iterator for TupleIterator {
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

    fn might_have_side_effects(&self) -> bool {
        false
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
pub struct StringIterator {
    data: ValueString,
    index: usize,
    end: usize,
}

impl StringIterator {
    pub fn new(data: ValueString) -> Self {
        let end = data.len();
        Self {
            data,
            index: 0,
            end,
        }
    }

    fn as_slice(&self) -> &str {
        &self.data[self.index..self.end]
    }
}

impl KotoIterator for StringIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::new(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<ValueIteratorOutput> {
        let remaining = self.as_slice();
        match GraphemeCursor::new(remaining.len(), remaining.len(), true)
            .prev_boundary(remaining, self.index)
            .unwrap() // Safety: self.index will be on a grapheme boundary or at the string's end
        {
            Some(grapheme_start) => {
                let result = self.data
                    .with_bounds(grapheme_start..self.end)
                    .unwrap(); // Safety: Some(_) returned from next_boundary implies valid bounds
                self.end = grapheme_start;
                Some(ValueIteratorOutput::Value(Value::Str(result)))
            }
            None => None,
        }
    }
}

impl Iterator for StringIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.as_slice();
        match GraphemeCursor::new(0, remaining.len(), true)
            .next_boundary(remaining, 0)
            .unwrap() // Safety: self.index will be on a grapheme boundary or at the string's end
        {
            Some(grapheme_end) => {
                let result = self.data
                    .with_bounds(self.index..self.index + grapheme_end)
                    .unwrap(); // Safety: Some(_) returned from next_boundary implies valid bounds
                self.index += grapheme_end;
                Some(ValueIteratorOutput::Value(Value::Str(result)))
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.data[self.index..].len();
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

    fn might_have_side_effects(&self) -> bool {
        true
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

    fn might_have_side_effects(&self) -> bool {
        false
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

    fn might_have_side_effects(&self) -> bool {
        false
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
