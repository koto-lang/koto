//! Adapators used by the `iterator` core library module

use super::collect_pair;
use crate::{prelude::*, Error, KIteratorOutput as Output, Result};
use std::{collections::VecDeque, result::Result as StdResult};
use thiserror::Error;

/// An iterator that links the output of two iterators together in a chained sequence
pub struct Chain {
    iter_a: Option<KIterator>,
    iter_b: KIterator,
}

impl Chain {
    /// Creates a [Chain] adapator from two iterators
    pub fn new(iter_a: KIterator, iter_b: KIterator) -> Self {
        Self {
            iter_a: Some(iter_a),
            iter_b,
        }
    }
}

impl KotoIterator for Chain {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter_a: match &self.iter_a {
                Some(iter) => Some(iter.make_copy()?),
                None => None,
            },
            iter_b: self.iter_b.make_copy()?,
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
    chunk_size: usize,
}

impl Chunks {
    /// Creates a [Chunks] adapator
    pub fn new(iter: KIterator, chunk_size: usize) -> StdResult<Self, ChunksError> {
        if chunk_size < 1 {
            Err(ChunksError::ChunkSizeMustBeAtLeastOne)
        } else {
            Ok(Self { iter, chunk_size })
        }
    }
}

impl KotoIterator for Chunks {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            chunk_size: self.chunk_size,
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Chunks {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk = None;

        for output in self.iter.clone().take(self.chunk_size) {
            match KValue::try_from(output) {
                Ok(value) => chunk
                    .get_or_insert_with(|| Vec::with_capacity(self.chunk_size))
                    .push(value),
                Err(error) => return Some(Output::Error(error)),
            }
        }

        chunk.map(|chunk| KTuple::from(chunk).into())
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
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum ChunksError {
    #[error("the chunk size must be at least 1")]
    ChunkSizeMustBeAtLeastOne,
}

/// An iterator that cycles through the adapted iterator infinitely
pub struct Cycle {
    iter: KIterator,
    cache: Vec<KValue>,
    cycle_index: usize,
}

impl Cycle {
    /// Creates a new [Cycle] adaptor
    pub fn new(iter: KIterator) -> Self {
        let (lower_bound, _) = iter.size_hint();
        let size_hint = if lower_bound < usize::MAX {
            lower_bound
        } else {
            0
        };

        Self {
            iter,
            cache: Vec::with_capacity(size_hint),
            cycle_index: 0,
        }
    }
}

impl KotoIterator for Cycle {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            cache: self.cache.clone(),
            cycle_index: self.cycle_index,
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Cycle {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(output) = self.iter.next() {
            match KValue::try_from(output) {
                Ok(value) => {
                    self.cache.push(value.clone());
                    Some(value.into())
                }
                Err(error) => Some(Output::Error(error)),
            }
        } else if self.cache.is_empty() {
            None
        } else {
            if self.cycle_index == self.cache.len() {
                self.cycle_index = 0;
            }
            let result = self.cache[self.cycle_index].clone();
            self.cycle_index += 1;
            Some(result.into())
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
    iter: KIterator,
    function: KValue,
    vm: KotoVm,
}

impl Each {
    /// Creates a new [Each] adaptor
    pub fn new(iter: KIterator, function: KValue, vm: KotoVm) -> Self {
        Self { iter, function, vm }
    }

    fn map_output(&mut self, output: Output) -> Output {
        let function = self.function.clone();
        let functor_result = match output {
            Output::Value(value) => self.vm.call_function(function, value),
            Output::ValuePair(a, b) => self.vm.call_function(function, CallArgs::AsTuple(&[a, b])),
            other => return other,
        };
        match functor_result {
            Ok(result) => Output::Value(result),
            Err(error) => Output::Error(error),
        }
    }
}

impl KotoIterator for Each {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
    index: usize,
}

impl Enumerate {
    /// Creates a new [Enumerate] adaptor
    pub fn new(iter: KIterator) -> Self {
        Self { iter, index: 0 }
    }
}

impl KotoIterator for Enumerate {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            index: self.index,
        };
        Ok(KIterator::new(result))
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
    vm: KotoVm,
    iter: KIterator,
    nested: Option<KIterator>,
}

impl Flatten {
    /// Creates a new [Flatten] adaptor
    pub fn new(iter: KIterator, vm: KotoVm) -> Self {
        Self {
            vm,
            iter,
            nested: None,
        }
    }
}

impl KotoIterator for Flatten {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            vm: self.vm.spawn_shared_vm(),
            iter: self.iter.make_copy()?,
            nested: match &self.nested {
                Some(nested) => Some(nested.make_copy()?),
                None => None,
            },
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator: KValue,
}

impl Intersperse {
    /// Creates a new [Intersperse] adaptor
    pub fn new(iter: KIterator, separator: KValue) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator,
        }
    }
}

impl KotoIterator for Intersperse {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator: self.separator.clone(),
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator_function: KValue,
    vm: KotoVm,
}

impl IntersperseWith {
    /// Creates a new [IntersperseWith] adaptor
    pub fn new(iter: KIterator, separator_function: KValue, vm: KotoVm) -> Self {
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
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator_function: self.separator_function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        Ok(KIterator::new(result))
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
                    match self.vm.call_function(self.separator_function.clone(), &[]) {
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

fn intersperse_size_hint(iter: &KIterator, next_is_separator: bool) -> (usize, Option<usize>) {
    let (lower, upper) = iter.size_hint();
    let offset = !next_is_separator as usize;

    let lower = lower.saturating_sub(offset).saturating_add(lower);
    let upper = upper.and_then(|upper| upper.saturating_sub(offset).checked_add(upper));

    (lower, upper)
}

/// An iterator that skips over values that fail a predicate, and keeps those that pass
pub struct Keep {
    iter: KIterator,
    predicate: KValue,
    vm: KotoVm,
}

impl Keep {
    /// Creates a new [Keep] adaptor
    pub fn new(iter: KIterator, predicate: KValue, vm: KotoVm) -> Self {
        Self {
            iter,
            predicate,
            vm,
        }
    }
}

impl KotoIterator for Keep {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Keep {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        for output in &mut self.iter {
            let predicate = self.predicate.clone();
            let predicate_result = match &output {
                Output::Value(value) => self.vm.call_function(predicate, value.clone()),
                Output::ValuePair(a, b) => self
                    .vm
                    .call_function(predicate, CallArgs::AsTuple(&[a.clone(), b.clone()])),
                error @ Output::Error(_) => return Some(error.clone()),
            };

            let result = match predicate_result {
                Ok(KValue::Bool(false)) => continue,
                Ok(KValue::Bool(true)) => output,
                Ok(unexpected) => Output::Error(
                    format!(
                    "iterator.keep: Expected a Bool to be returned from the predicate, found '{}'",
                    unexpected.type_as_string()
                )
                    .into(),
                ),
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
    iter: KIterator,
}

impl PairFirst {
    /// Creates a new [PairFirst] adaptor
    pub fn new(iter: KIterator) -> Self {
        Self { iter }
    }
}

impl KotoIterator for PairFirst {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
}

impl PairSecond {
    /// Creates a new [PairSecond] adaptor
    pub fn new(iter: KIterator) -> Self {
        Self { iter }
    }
}

impl KotoIterator for PairSecond {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
        };
        Ok(KIterator::new(result))
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
    iter: KIterator,
}

impl Reversed {
    /// Creates a new [Reversed] adaptor
    pub fn new(iter: KIterator) -> StdResult<Self, ReversedError> {
        if iter.is_bidirectional() {
            Ok(Self {
                iter: iter.make_copy().map_err(ReversedError::CopyError)?,
            })
        } else {
            Err(ReversedError::IteratorIsntReversible)
        }
    }
}

impl KotoIterator for Reversed {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
        };
        Ok(KIterator::new(result))
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
#[derive(Debug, Error)]
pub enum ReversedError {
    #[error("the provided iterator isn't bidirectional")]
    IteratorIsntReversible,
    #[error("failed to copy the iterator ('{0}')")]
    CopyError(Error),
}

/// An iterator that yields the next value from the input, and then steps forward by
pub struct Step {
    iter: KIterator,
    step: u64,
}

impl Step {
    /// Creates a new [Step] adaptor
    pub fn new(iter: KIterator, step: u64) -> StdResult<Self, StepError> {
        if step == 0 {
            Err(StepError::StepCantBeZero)
        } else {
            Ok(Self { iter, step })
        }
    }
}

impl KotoIterator for Step {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            step: self.step,
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Step {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.iter.next();
        for _ in 0..self.step - 1 {
            self.iter.next();
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let step = self.step as usize;
        let (lower, upper) = self.iter.size_hint();
        (lower / step, upper.map(|upper| upper / step))
    }
}

/// An error that can be returned by [Step::new]
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum StepError {
    #[error("the step size must be greater than zero")]
    StepCantBeZero,
}

/// An iterator that takes up to N values from the adapted iterator, and then stops
pub struct Take {
    iter: KIterator,
    remaining: usize,
}

impl Take {
    /// Creates a new [Take] adaptor
    pub fn new(iter: KIterator, count: usize) -> Self {
        Self {
            iter,
            remaining: count,
        }
    }
}

impl KotoIterator for Take {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            remaining: self.remaining,
        };
        Ok(KIterator::new(result))
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

/// An adaptor that yields values from an iterator while they pass a predicate
pub struct TakeWhile {
    iter: KIterator,
    predicate: KValue,
    vm: KotoVm,
    finished: bool,
}

impl TakeWhile {
    /// Creates a new [Keep] adaptor
    pub fn new(iter: KIterator, predicate: KValue, vm: KotoVm) -> Self {
        Self {
            iter,
            predicate,
            vm,
            finished: false,
        }
    }
}

impl KotoIterator for TakeWhile {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
            finished: self.finished,
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for TakeWhile {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let iter_output = self.iter.next()?;
        let predicate = self.predicate.clone();
        let predicate_result = match &iter_output {
            Output::Value(value) => self.vm.call_function(predicate, value.clone()),
            Output::ValuePair(a, b) => self
                .vm
                .call_function(predicate, CallArgs::AsTuple(&[a.clone(), b.clone()])),
            error @ Output::Error(_) => return Some(error.clone()),
        };

        let result = match predicate_result {
            Ok(KValue::Bool(true)) => iter_output,
            Ok(KValue::Bool(false)) => {
                self.finished = true;
                return None;
            }
            Ok(unexpected) => Output::Error(
                format!(
                    "expected a Bool to be returned from the predicate, found '{}'",
                    unexpected.type_as_string()
                )
                .into(),
            ),
            Err(error) => Output::Error(error),
        };

        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_lower, upper) = self.iter.size_hint();
        (0, upper)
    }
}

/// An iterator that splits the incoming iterator into overlapping iterators of size N
pub struct Windows {
    iter: KIterator,
    cache: VecDeque<KValue>,
    window_size: usize,
}

impl Windows {
    /// Creates a new [Windows] adaptor
    pub fn new(iter: KIterator, window_size: usize) -> StdResult<Self, WindowsError> {
        if window_size < 1 {
            Err(WindowsError::WindowSizeMustBeAtLeastOne)
        } else {
            Ok(Self {
                iter,
                cache: VecDeque::with_capacity(window_size),
                window_size,
            })
        }
    }
}

impl KotoIterator for Windows {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            cache: self.cache.clone(),
            window_size: self.window_size,
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Windows {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.cache.pop_front();

        while self.cache.len() < self.window_size {
            let Some(output) = self.iter.next() else {
                break;
            };

            match KValue::try_from(output) {
                Ok(value) => self.cache.push_back(value),
                Err(error) => return Some(Output::Error(error)),
            }
        }

        if self.cache.len() == self.window_size {
            let result: Vec<_> = self.cache.iter().cloned().collect();
            Some(KTuple::from(result).into())
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
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum WindowsError {
    #[error("the window size must be at least 1")]
    WindowSizeMustBeAtLeastOne,
}

/// An iterator that combines the output of two iterators, 'zipping' output pairs together
pub struct Zip {
    iter_a: KIterator,
    iter_b: KIterator,
}

impl Zip {
    /// Creates a new [Zip] adaptor
    pub fn new(iter_a: KIterator, iter_b: KIterator) -> Self {
        Self { iter_a, iter_b }
    }
}

impl KotoIterator for Zip {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter_a: self.iter_a.make_copy()?,
            iter_b: self.iter_b.make_copy()?,
        };
        Ok(KIterator::new(result))
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
