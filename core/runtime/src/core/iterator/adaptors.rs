//! Adapators used by the `iterator` core library module

use {
    super::collect_pair,
    crate::{prelude::*, ValueIteratorOutput as Output},
    std::{error, fmt},
};

/// An iterator that links the output of two iterators together in a chained sequence
pub struct Chain {
    iter_a: Option<ValueIterator>,
    iter_b: ValueIterator,
}

impl Chain {
    /// Creates a [Chain] adapator from two iterators
    pub fn new(iter_a: ValueIterator, iter_b: ValueIterator) -> Self {
        Self {
            iter_a: Some(iter_a),
            iter_b,
        }
    }
}

impl KotoIterator for Chain {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter_a: self.iter_a.as_ref().map(|iter| iter.make_copy()),
            iter_b: self.iter_b.make_copy(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Chain {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter_a {
            Some(ref mut iter) => match iter.next() {
                output @ Some(_) => output,
                None => {
                    self.iter_a = None;
                    self.iter_b.next()
                }
            },
            None => self.iter_b.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.iter_a {
            Some(iter_a) => {
                let (lower_a, upper_a) = iter_a.size_hint();
                let (lower_b, upper_b) = self.iter_b.size_hint();

                let lower = lower_a.saturating_add(lower_b);
                let upper = match (upper_a, upper_b) {
                    (Some(a), Some(b)) => a.checked_add(b),
                    _ => None,
                };

                (lower, upper)
            }
            None => self.iter_b.size_hint(),
        }
    }
}

/// An iterator that splits the incoming iterator into iterators of size N
pub struct Chunks {
    iter: ValueIterator,
    chunk_size: usize,
}

impl Chunks {
    /// Creates a [Chunks] adapator
    pub fn new(iter: ValueIterator, chunk_size: usize) -> Result<Self, ChunksError> {
        if chunk_size < 1 {
            Err(ChunksError::ChunkSizeMustBeAtLeastOne)
        } else {
            Ok(Self { iter, chunk_size })
        }
    }
}

impl KotoIterator for Chunks {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            chunk_size: self.chunk_size,
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Chunks {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk = None;

        for output in self.iter.clone().take(self.chunk_size) {
            match Value::try_from(output) {
                Ok(value) => chunk
                    .get_or_insert_with(|| Vec::with_capacity(self.chunk_size))
                    .push(value),
                Err(error) => return Some(Output::Error(error)),
            }
        }

        chunk.map(|chunk| ValueTuple::from(chunk).into())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();

        let lower = {
            let mut chunk_count = lower / self.chunk_size;
            if lower % self.chunk_size > 0 {
                chunk_count += 1;
            }
            chunk_count
        };

        let upper = upper.map(|upper| {
            let mut chunk_count = upper / self.chunk_size;
            if upper % self.chunk_size > 0 {
                chunk_count += 1;
            }
            chunk_count
        });

        (lower, upper)
    }
}

/// An error that can be returned by [Chunks::new]
#[allow(missing_docs)]
pub enum ChunksError {
    ChunkSizeMustBeAtLeastOne,
}

impl fmt::Display for ChunksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunksError::ChunkSizeMustBeAtLeastOne => {
                write!(f, "the chunk size must be at least 1")
            }
        }
    }
}

impl fmt::Debug for ChunksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl error::Error for ChunksError {}

/// An iterator that cycles through the adapted iterator infinitely
pub struct Cycle {
    iter: ValueIterator,
    current_cycle: ValueIterator,
}

impl Cycle {
    /// Creates a new [Cycle] adaptor
    pub fn new(iterator: ValueIterator) -> Self {
        Self {
            iter: iterator.make_copy(),
            current_cycle: iterator,
        }
    }
}

impl KotoIterator for Cycle {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            current_cycle: self.current_cycle.make_copy(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Cycle {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_cycle.next() {
            None => {
                self.current_cycle = self.iter.make_copy();
                self.current_cycle.next()
            }
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.iter.size_hint() {
            // If the incoming iterator is empty, this iterator is empty
            (0, Some(0)) => (0, Some(0)),
            // Even if we know the size hint of the incoming iterator we can not know
            // the upper bound of this iterator since it is infinite
            (0, _) => (0, None),
            // An infinite iterator has no upper bound
            // and the maximum possible lower bound
            // https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.size_hint
            _ => (usize::MAX, None),
        }
    }
}

/// An iterator that runs a function on each output value from the adapted iterator
pub struct Each {
    iter: ValueIterator,
    function: Value,
    vm: Vm,
}

impl Each {
    /// Creates a new [Each] adaptor
    pub fn new(iter: ValueIterator, function: Value, vm: Vm) -> Self {
        Self { iter, function, vm }
    }

    fn map_output(&mut self, output: Output) -> Output {
        let function = self.function.clone();
        let functor_result = match output {
            Output::Value(value) => self.vm.run_function(function, CallArgs::Single(value)),
            Output::ValuePair(a, b) => self.vm.run_function(function, CallArgs::AsTuple(&[a, b])),
            other => return other,
        };
        match functor_result {
            Ok(result) => Output::Value(result),
            Err(error) => Output::Error(error),
        }
    }
}

impl KotoIterator for Each {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::new(result)
    }

    fn is_bidirectional(&self) -> bool {
        self.iter.is_bidirectional()
    }

    fn next_back(&mut self) -> Option<Output> {
        self.iter.next_back().map(|output| self.map_output(output))
    }
}

impl Iterator for Each {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|output| self.map_output(output))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that attaches an enumerated iteration position to each value
pub struct Enumerate {
    iter: ValueIterator,
    index: usize,
}

impl Enumerate {
    /// Creates a new [Enumerate] adaptor
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter, index: 0 }
    }
}

impl KotoIterator for Enumerate {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            index: self.index,
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Enumerate {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self
            .iter
            .next()
            .map(collect_pair) // Collect pairs for the RHS of the enumeration
            .map(|output| match output {
                // The output can be a ValuePair
                Output::Value(value) => Output::ValuePair(self.index.into(), value),
                other => other,
            });
        self.index += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that flattens the output of nested iterators
pub struct Flatten {
    vm: Vm,
    iter: ValueIterator,
    nested: Option<ValueIterator>,
}

impl Flatten {
    /// Creates a new [Flatten] adaptor
    pub fn new(iter: ValueIterator, vm: Vm) -> Self {
        Self {
            vm,
            iter,
            nested: None,
        }
    }
}

impl KotoIterator for Flatten {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            vm: self.vm.spawn_shared_vm(),
            iter: self.iter.make_copy(),
            nested: self.nested.as_ref().map(|nested| nested.make_copy()),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Flatten {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(nested) = &mut self.nested {
                if let result @ Some(_) = nested.next() {
                    return result;
                }
            }

            match self.iter.next().map(collect_pair) {
                Some(Output::Value(iterable)) if iterable.is_iterable() => {
                    match self.vm.make_iterator(iterable) {
                        Ok(nested) => {
                            self.nested = Some(nested);
                            continue;
                        }
                        Err(error) => return Some(Output::Error(error)),
                    }
                }
                other => return other,
            }
        }
    }
}

/// An iterator that inserts a separator value between each output value from the adapted iterator
pub struct Intersperse {
    iter: ValueIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator: Value,
}

impl Intersperse {
    /// Creates a new [Intersperse] adaptor
    pub fn new(iter: ValueIterator, separator: Value) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator,
        }
    }
}

impl KotoIterator for Intersperse {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator: self.separator.clone(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Intersperse {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.peeked.take().or_else(|| self.iter.next());

        if next.is_some() {
            let result = if self.next_is_separator {
                self.peeked = next;
                Some(Output::Value(self.separator.clone()))
            } else {
                next
            };

            self.next_is_separator = !self.next_is_separator;
            result
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        intersperse_size_hint(&self.iter, self.next_is_separator)
    }
}

/// An iterator that inserts a separator value between each output value from the adapted iterator
///
/// The separator value is the result of calling a provided separator function.
pub struct IntersperseWith {
    iter: ValueIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator_function: Value,
    vm: Vm,
}

impl IntersperseWith {
    /// Creates a new [IntersperseWith] adaptor
    pub fn new(iter: ValueIterator, separator_function: Value, vm: Vm) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator_function,
            vm,
        }
    }
}

impl KotoIterator for IntersperseWith {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator_function: self.separator_function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for IntersperseWith {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.peeked.take().or_else(|| self.iter.next());

        if next.is_some() {
            let result = if self.next_is_separator {
                self.peeked = next;
                Some(
                    match self
                        .vm
                        .run_function(self.separator_function.clone(), CallArgs::None)
                    {
                        Ok(result) => Output::Value(result),
                        Err(error) => Output::Error(error),
                    },
                )
            } else {
                next
            };

            self.next_is_separator = !self.next_is_separator;
            result
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        intersperse_size_hint(&self.iter, self.next_is_separator)
    }
}

fn intersperse_size_hint(iter: &ValueIterator, next_is_separator: bool) -> (usize, Option<usize>) {
    let (lower, upper) = iter.size_hint();
    let offset = !next_is_separator as usize;

    let lower = lower.saturating_sub(offset).saturating_add(lower);
    let upper = upper.and_then(|upper| upper.saturating_sub(offset).checked_add(upper));

    (lower, upper)
}

/// An iterator that skips over values that fail a predicate, and keeps those that pass
pub struct Keep {
    iter: ValueIterator,
    predicate: Value,
    vm: Vm,
}

impl Keep {
    /// Creates a new [Keep] adaptor
    pub fn new(iter: ValueIterator, predicate: Value, vm: Vm) -> Self {
        Self {
            iter,
            predicate,
            vm,
        }
    }
}

impl KotoIterator for Keep {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Keep {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        for output in &mut self.iter {
            let predicate = self.predicate.clone();
            let predicate_result = match &output {
                Output::Value(value) => self
                    .vm
                    .run_function(predicate, CallArgs::Single(value.clone())),
                Output::ValuePair(a, b) => self
                    .vm
                    .run_function(predicate, CallArgs::AsTuple(&[a.clone(), b.clone()])),
                error @ Output::Error(_) => return Some(error.clone()),
            };

            let result = match predicate_result {
                Ok(Value::Bool(false)) => continue,
                Ok(Value::Bool(true)) => output,
                Ok(unexpected) => Output::Error(make_runtime_error!(format!(
                    "iterator.keep: Expected a Bool to be returned from the predicate, found '{}'",
                    unexpected.type_as_string()
                ))),
                Err(error) => Output::Error(error),
            };

            return Some(result);
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_lower, upper) = self.iter.size_hint();
        (0, upper)
    }
}

/// An iterator that outputs the first element from any ValuePairs
pub struct PairFirst {
    iter: ValueIterator,
}

impl PairFirst {
    /// Creates a new [PairFirst] adaptor
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter }
    }
}

impl KotoIterator for PairFirst {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for PairFirst {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Output::ValuePair(first, _)) => Some(Output::Value(first)),
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that outputs the second element from any ValuePairs
pub struct PairSecond {
    iter: ValueIterator,
}

impl PairSecond {
    /// Creates a new [PairSecond] adaptor
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter }
    }
}

impl KotoIterator for PairSecond {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for PairSecond {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Output::ValuePair(_, second)) => Some(Output::Value(second)),
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator adaptor that reverses the output of the input iterator
pub struct Reversed {
    iter: ValueIterator,
}

impl Reversed {
    /// Creates a new [Reversed] adaptor
    pub fn new(iter: ValueIterator) -> Result<Self, ReversedError> {
        if iter.is_bidirectional() {
            Ok(Self {
                iter: iter.make_copy(),
            })
        } else {
            Err(ReversedError::IteratorIsntReversible)
        }
    }
}

impl KotoIterator for Reversed {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
        };
        ValueIterator::new(result)
    }

    fn is_bidirectional(&self) -> bool {
        true
    }

    fn next_back(&mut self) -> Option<Output> {
        self.iter.next()
    }
}

impl Iterator for Reversed {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An error that can be returned by [Reversed::new]
#[allow(missing_docs)]
pub enum ReversedError {
    IteratorIsntReversible,
}

impl fmt::Display for ReversedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReversedError::IteratorIsntReversible => {
                write!(f, "the provided iterator isn't bidirectional")
            }
        }
    }
}

impl fmt::Debug for ReversedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl error::Error for ReversedError {}

/// An iterator that takes up to N values from the adapted iterator, and then stops
pub struct Take {
    iter: ValueIterator,
    remaining: usize,
}

impl Take {
    /// Creates a new [Take] adaptor
    pub fn new(iter: ValueIterator, count: usize) -> Self {
        Self {
            iter,
            remaining: count,
        }
    }
}

impl KotoIterator for Take {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            remaining: self.remaining,
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Take {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            self.iter.next()
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (
            lower.min(self.remaining),
            upper.map(|upper| upper.min(self.remaining)),
        )
    }
}

/// An iterator that splits the incoming iterator into overlapping iterators of size N
pub struct Windows {
    iter: ValueIterator,
    end_iter: ValueIterator,
    window_size: usize,
}

impl Windows {
    /// Creates a new [Windows] adaptor
    pub fn new(iter: ValueIterator, window_size: usize) -> Result<Self, WindowsError> {
        if window_size < 1 {
            Err(WindowsError::WindowSizeMustBeAtLeastOne)
        } else {
            let mut end_iter = iter.make_copy();
            // Skip the end iterator to 'one before the last' of the first window
            if window_size > 1 {
                end_iter.nth(window_size - 2);
            }

            Ok(Self {
                iter,
                end_iter,
                window_size,
            })
        }
    }
}

impl KotoIterator for Windows {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            end_iter: self.end_iter.make_copy(),
            window_size: self.window_size,
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Windows {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        // The end iterator is positioned just before the end of the window,
        // if next() outputs a value then there's at least one more window.
        if self.end_iter.next().is_some() {
            // Make the next window by using a Take adaptor.
            let window_iter = Take::new(self.iter.make_copy(), self.window_size);

            // Move the input iterator to the start of the next window
            self.iter.next();

            Some(Output::Value(ValueIterator::new(window_iter).into()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        let lower = lower.saturating_sub(self.window_size) + 1;
        let upper = upper.map(|upper| upper.saturating_sub(self.window_size) + 1);
        (lower, upper)
    }
}

/// An error that can be returned by [Windows::new]
#[allow(missing_docs)]
pub enum WindowsError {
    WindowSizeMustBeAtLeastOne,
}

impl fmt::Display for WindowsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowsError::WindowSizeMustBeAtLeastOne => {
                write!(f, "the window size must be at least 1")
            }
        }
    }
}

impl fmt::Debug for WindowsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl error::Error for WindowsError {}

/// An iterator that combines the output of two iterators, 'zipping' output pairs together
pub struct Zip {
    iter_a: ValueIterator,
    iter_b: ValueIterator,
}

impl Zip {
    /// Creates a new [Zip] adaptor
    pub fn new(iter_a: ValueIterator, iter_b: ValueIterator) -> Self {
        Self { iter_a, iter_b }
    }
}

impl KotoIterator for Zip {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter_a: self.iter_a.make_copy(),
            iter_b: self.iter_b.make_copy(),
        };
        ValueIterator::new(result)
    }
}

impl Iterator for Zip {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter_a.next().map(collect_pair) {
            Some(Output::Value(value_a)) => match self.iter_b.next().map(collect_pair) {
                Some(Output::Value(value_b)) => Some(Output::ValuePair(value_a, value_b)),
                error @ Some(Output::Error(_)) => error,
                _ => None,
            },
            error @ Some(Output::Error(_)) => error,
            _ => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower_a, upper_a) = self.iter_a.size_hint();
        let (lower_b, upper_b) = self.iter_b.size_hint();

        let lower = lower_a.min(lower_b);
        let upper = match (upper_a, upper_b) {
            (Some(upper_a), Some(upper_b)) => Some(upper_a.min(upper_b)),
            _ => None,
        };

        (lower, upper)
    }
}

// For tests, see runtime/tests/iterator_tests.rs
