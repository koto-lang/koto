//! Adapators used by the `iterator` core library module

use super::collect_pair;
use crate::{Error, ErrorKind, InstructionFrame, KIteratorOutput as Output, Result, prelude::*};
use std::{collections::VecDeque, mem::take, result::Result as StdResult, task::Context};
use thiserror::Error;

fn call_function_with_output_as_task(
    vm: &mut KotoVm,
    function: KValue,
    output: &Output,
) -> Result<KTask> {
    match output {
        Output::Value(value) => vm.call_function_as_task(function, value.clone()),
        Output::ValuePair(a, b) => {
            vm.call_function_as_task(function, CallArgs::AsTuple(&[a.clone(), b.clone()]))
        }
        Output::Error(error) => Err(error.clone()),
    }
}

fn poll_task(
    vm: &KotoVm,
    task: &mut KTask,
    context: &mut Context<'_>,
    error_frame: &InstructionFrame,
) -> KIteratorNext {
    match poll_task_value(vm, task, context, error_frame) {
        TaskValuePoll::Ready(result) => KIteratorNext::Output(Output::Value(result)),
        TaskValuePoll::Pending => KIteratorNext::Pending,
        TaskValuePoll::Error(error) => KIteratorNext::Output(error),
    }
}

fn poll_task_value(
    _vm: &KotoVm,
    task: &mut KTask,
    context: &mut Context<'_>,
    error_frame: &InstructionFrame,
) -> TaskValuePoll {
    match task.poll_with_context(context) {
        Ok(KTaskPoll::Ready(result)) => TaskValuePoll::Ready(result),
        Ok(KTaskPoll::Pending) => TaskValuePoll::Pending,
        Err(mut error) => {
            error.extend_trace(error_frame.clone());
            TaskValuePoll::Error(Output::Error(error))
        }
    }
}

fn poll_task_value_sync(
    vm: &KotoVm,
    task: &mut KTask,
    error_frame: &InstructionFrame,
) -> TaskValuePoll {
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);

    loop {
        match poll_task_value(vm, task, &mut context, error_frame) {
            TaskValuePoll::Pending => std::thread::yield_now(),
            result => return result,
        }
    }
}

fn call_function_with_output_sync(
    vm: &mut KotoVm,
    function: KValue,
    output: &Output,
    error_frame: &InstructionFrame,
) -> Output {
    let mut task = match call_function_with_output_as_task(vm, function, output) {
        Ok(task) => task,
        Err(mut error) => {
            error.extend_trace(error_frame.clone());
            return Output::Error(error);
        }
    };

    match poll_task_value_sync(vm, &mut task, error_frame) {
        TaskValuePoll::Ready(result) => Output::Value(result),
        TaskValuePoll::Pending => unreachable!(),
        TaskValuePoll::Error(error) => error,
    }
}

fn bool_result_to_next(
    result: KValue,
    iter_output: Output,
    error_frame: &InstructionFrame,
) -> BoolNextResult {
    match result {
        KValue::Bool(false) => BoolNextResult::False,
        KValue::Bool(true) => BoolNextResult::True(iter_output),
        unexpected => BoolNextResult::Error(Output::Error(Error::with_error_frame(
            ErrorKind::UnexpectedType {
                expected: "Bool from the predicate".into(),
                unexpected,
            },
            error_frame.clone(),
        ))),
    }
}

fn unexpected_iterator_output(unexpected: KValue, error_frame: &InstructionFrame) -> Output {
    Output::Error(Error::with_error_frame(
        ErrorKind::UnexpectedType {
            expected: "Iterator".into(),
            unexpected,
        },
        error_frame.clone(),
    ))
}

fn unexpected_iterator_result(unexpected: KValue, error_frame: &InstructionFrame) -> KIteratorNext {
    KIteratorNext::Output(unexpected_iterator_output(unexpected, error_frame))
}

enum BoolNextResult {
    True(Output),
    False,
    Error(Output),
}

enum TaskValuePoll {
    Ready(KValue),
    Pending,
    Error(Output),
}

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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        match self.iter_a {
            Some(ref mut iter) => match iter.next_output_with_context(context) {
                output @ KIteratorNext::Output(_) => output,
                KIteratorNext::Pending => KIteratorNext::Pending,
                KIteratorNext::Done => {
                    self.iter_a = None;
                    self.iter_b.next_output_with_context(context)
                }
            },
            None => self.iter_b.next_output_with_context(context),
        }
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
    chunk: Vec<KValue>,
}

impl Chunks {
    /// Creates a [Chunks] adapator
    pub fn new(iter: KIterator, chunk_size: usize) -> StdResult<Self, ChunksError> {
        if chunk_size < 1 {
            Err(ChunksError::ChunkSizeMustBeAtLeastOne)
        } else {
            Ok(Self {
                iter,
                chunk_size,
                chunk: Vec::with_capacity(chunk_size),
            })
        }
    }
}

impl KotoIterator for Chunks {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            chunk_size: self.chunk_size,
            chunk: self.chunk.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        while self.chunk.len() < self.chunk_size {
            match self.iter.next_output_with_context(context) {
                KIteratorNext::Output(output) => match KValue::try_from(output) {
                    Ok(value) => self.chunk.push(value),
                    Err(error) => {
                        self.chunk.clear();
                        return KIteratorNext::Output(Output::Error(error));
                    }
                },
                KIteratorNext::Pending => return KIteratorNext::Pending,
                KIteratorNext::Done => break,
            }
        }

        if self.chunk.is_empty() {
            KIteratorNext::Done
        } else {
            let chunk = std::mem::replace(&mut self.chunk, Vec::with_capacity(self.chunk_size));
            KIteratorNext::Output(KTuple::from(chunk).into())
        }
    }
}

impl Iterator for Chunks {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        while self.chunk.len() < self.chunk_size {
            let Some(output) = self.iter.next() else {
                break;
            };

            match KValue::try_from(output) {
                Ok(value) => self.chunk.push(value),
                Err(error) => return Some(Output::Error(error)),
            }
        }

        if self.chunk.is_empty() {
            None
        } else {
            let chunk = std::mem::replace(&mut self.chunk, Vec::with_capacity(self.chunk_size));
            Some(KTuple::from(chunk).into())
        }
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        match self.iter.next_output_with_context(context) {
            KIteratorNext::Output(output) => match KValue::try_from(output) {
                Ok(value) => {
                    self.cache.push(value.clone());
                    KIteratorNext::Output(value.into())
                }
                Err(error) => KIteratorNext::Output(Output::Error(error)),
            },
            KIteratorNext::Pending => KIteratorNext::Pending,
            KIteratorNext::Done if self.cache.is_empty() => KIteratorNext::Done,
            KIteratorNext::Done => {
                if self.cycle_index == self.cache.len() {
                    self.cycle_index = 0;
                }
                let result = self.cache[self.cycle_index].clone();
                self.cycle_index += 1;
                KIteratorNext::Output(result.into())
            }
        }
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
    error_frame: InstructionFrame,
    pending_function: Option<KTask>,
}

impl Each {
    /// Creates a new [Each] adaptor
    pub fn new(iter: KIterator, function: KValue, vm: &KotoVm) -> Self {
        Self {
            iter,
            function,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            pending_function: None,
        }
    }

    fn map_output(&mut self, output: Output) -> Output {
        if matches!(output, Output::Error(_)) {
            return output;
        }

        call_function_with_output_sync(
            &mut self.vm,
            self.function.clone(),
            &output,
            &self.error_frame,
        )
    }
}

impl KotoIterator for Each {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
            error_frame: self.error_frame.clone(),
            pending_function: self.pending_function.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if let Some(mut task) = self.pending_function.take() {
            let result = poll_task(&self.vm, &mut task, context, &self.error_frame);
            if matches!(result, KIteratorNext::Pending) {
                self.pending_function = Some(task);
            }
            return result;
        }

        let output = match self.iter.next_output_with_context(context) {
            KIteratorNext::Output(Output::Error(error)) => {
                return KIteratorNext::Output(Output::Error(error));
            }
            KIteratorNext::Output(output) => output,
            other => return other,
        };

        match call_function_with_output_as_task(&mut self.vm, self.function.clone(), &output) {
            Ok(mut task) => {
                let result = poll_task(&self.vm, &mut task, context, &self.error_frame);
                if matches!(result, KIteratorNext::Pending) {
                    self.pending_function = Some(task);
                }
                result
            }
            Err(mut error) => {
                error.extend_trace(self.error_frame.clone());
                KIteratorNext::Output(Output::Error(error))
            }
        }
    }

    fn is_bidirectional(&self) -> bool {
        self.iter.is_bidirectional()
    }

    fn next_back(&mut self) -> Option<Output> {
        self.iter.next_back().map(|output| self.map_output(output))
    }

    fn next_back_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if let Some(mut task) = self.pending_function.take() {
            let result = poll_task(&self.vm, &mut task, context, &self.error_frame);
            if matches!(result, KIteratorNext::Pending) {
                self.pending_function = Some(task);
            }
            return result;
        }

        let output = match self.iter.next_back_output_with_context(context) {
            KIteratorNext::Output(Output::Error(error)) => {
                return KIteratorNext::Output(Output::Error(error));
            }
            KIteratorNext::Output(output) => output,
            other => return other,
        };

        match call_function_with_output_as_task(&mut self.vm, self.function.clone(), &output) {
            Ok(mut task) => {
                let result = poll_task(&self.vm, &mut task, context, &self.error_frame);
                if matches!(result, KIteratorNext::Pending) {
                    self.pending_function = Some(task);
                }
                result
            }
            Err(mut error) => {
                error.extend_trace(self.error_frame.clone());
                KIteratorNext::Output(Output::Error(error))
            }
        }
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        match self
            .iter
            .next_output_with_context(context)
            .map(collect_pair)
        {
            KIteratorNext::Output(Output::Value(value)) => {
                let result = KIteratorNext::Output(Output::ValuePair(self.index.into(), value));
                self.index += 1;
                result
            }
            KIteratorNext::Output(other) => {
                self.index += 1;
                KIteratorNext::Output(other)
            }
            other => other,
        }
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
    iter: KIterator,
    nested: Option<KIterator>,
    pending_nested: Option<KTask>,
    vm: KotoVm,
    error_frame: InstructionFrame,
}

impl Flatten {
    /// Creates a new [Flatten] adaptor
    pub fn new(iter: KIterator, vm: &KotoVm) -> Self {
        Self {
            iter,
            nested: None,
            pending_nested: None,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
        }
    }
}

impl KotoIterator for Flatten {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            nested: match &self.nested {
                Some(nested) => Some(nested.make_copy()?),
                None => None,
            },
            pending_nested: self.pending_nested.clone(),
            vm: self.vm.spawn_shared_vm(),
            error_frame: self.error_frame.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        loop {
            if let Some(nested) = &mut self.nested {
                match nested.next_output_with_context(context) {
                    output @ KIteratorNext::Output(_) => return output,
                    KIteratorNext::Pending => return KIteratorNext::Pending,
                    KIteratorNext::Done => self.nested = None,
                }
            }

            if let Some(mut task) = self.pending_nested.take() {
                match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                    TaskValuePoll::Ready(KValue::Iterator(nested)) => {
                        self.nested = Some(nested);
                        continue;
                    }
                    TaskValuePoll::Ready(unexpected) => {
                        return unexpected_iterator_result(unexpected, &self.error_frame);
                    }
                    TaskValuePoll::Pending => {
                        self.pending_nested = Some(task);
                        return KIteratorNext::Pending;
                    }
                    TaskValuePoll::Error(error) => return KIteratorNext::Output(error),
                }
            }

            match self
                .iter
                .next_output_with_context(context)
                .map(collect_pair)
            {
                KIteratorNext::Output(Output::Value(iterable)) if iterable.is_iterable() => {
                    match self.vm.make_iterator_as_task(iterable) {
                        Ok(mut task) => {
                            match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                                TaskValuePoll::Ready(KValue::Iterator(nested)) => {
                                    self.nested = Some(nested);
                                    continue;
                                }
                                TaskValuePoll::Ready(unexpected) => {
                                    return unexpected_iterator_result(
                                        unexpected,
                                        &self.error_frame,
                                    );
                                }
                                TaskValuePoll::Pending => {
                                    self.pending_nested = Some(task);
                                    return KIteratorNext::Pending;
                                }
                                TaskValuePoll::Error(error) => {
                                    return KIteratorNext::Output(error);
                                }
                            }
                        }
                        Err(mut error) => {
                            error.extend_trace(self.error_frame.clone());
                            return KIteratorNext::Output(Output::Error(error));
                        }
                    }
                }
                other => return other,
            }
        }
    }
}

impl Iterator for Flatten {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);

        loop {
            if let Some(nested) = &mut self.nested
                && let result @ Some(_) = nested.next()
            {
                return result;
            }

            match self.iter.next().map(collect_pair) {
                Some(Output::Value(iterable)) if iterable.is_iterable() => {
                    let mut task = match self.vm.make_iterator_as_task(iterable) {
                        Ok(task) => task,
                        Err(mut error) => {
                            error.extend_trace(self.error_frame.clone());
                            return Some(Output::Error(error));
                        }
                    };

                    loop {
                        match poll_task_value(&self.vm, &mut task, &mut context, &self.error_frame)
                        {
                            TaskValuePoll::Ready(KValue::Iterator(nested)) => {
                                self.nested = Some(nested);
                                break;
                            }
                            TaskValuePoll::Ready(unexpected) => {
                                return Some(unexpected_iterator_output(
                                    unexpected,
                                    &self.error_frame,
                                ));
                            }
                            TaskValuePoll::Pending => std::thread::yield_now(),
                            TaskValuePoll::Error(error) => return Some(error),
                        }
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        let next = match self.peeked.take() {
            Some(output) => KIteratorNext::Output(output),
            None => self.iter.next_output_with_context(context),
        };

        match next {
            KIteratorNext::Output(output) => {
                let result = if self.next_is_separator {
                    self.peeked = Some(output);
                    KIteratorNext::Output(Output::Value(self.separator.clone()))
                } else {
                    KIteratorNext::Output(output)
                };

                self.next_is_separator = !self.next_is_separator;
                result
            }
            other => other,
        }
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
    error_frame: InstructionFrame,
    pending_separator: Option<KTask>,
}

impl IntersperseWith {
    /// Creates a new [IntersperseWith] adaptor
    pub fn new(iter: KIterator, separator_function: KValue, vm: &KotoVm) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator_function,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            pending_separator: None,
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
            error_frame: self.error_frame.clone(),
            pending_separator: self.pending_separator.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if let Some(mut task) = self.pending_separator.take() {
            match poll_task(&self.vm, &mut task, context, &self.error_frame) {
                KIteratorNext::Output(output @ Output::Value(_)) => {
                    self.next_is_separator = false;
                    return KIteratorNext::Output(output);
                }
                KIteratorNext::Output(output @ Output::Error(_)) => {
                    return KIteratorNext::Output(output);
                }
                KIteratorNext::Output(_) => unreachable!(),
                KIteratorNext::Pending => {
                    self.pending_separator = Some(task);
                    return KIteratorNext::Pending;
                }
                KIteratorNext::Done => unreachable!(),
            }
        }

        let next = match self.peeked.take() {
            Some(output) => KIteratorNext::Output(output),
            None => self.iter.next_output_with_context(context),
        };

        match next {
            KIteratorNext::Output(output) => {
                if self.next_is_separator {
                    self.peeked = Some(output);

                    match self
                        .vm
                        .call_function_as_task(self.separator_function.clone(), &[])
                    {
                        Ok(mut task) => {
                            match poll_task(&self.vm, &mut task, context, &self.error_frame) {
                                KIteratorNext::Output(output @ Output::Value(_)) => {
                                    self.next_is_separator = false;
                                    KIteratorNext::Output(output)
                                }
                                KIteratorNext::Output(output @ Output::Error(_)) => {
                                    KIteratorNext::Output(output)
                                }
                                KIteratorNext::Output(_) => unreachable!(),
                                KIteratorNext::Pending => {
                                    self.pending_separator = Some(task);
                                    KIteratorNext::Pending
                                }
                                KIteratorNext::Done => unreachable!(),
                            }
                        }
                        Err(mut error) => {
                            error.extend_trace(self.error_frame.clone());
                            KIteratorNext::Output(Output::Error(error))
                        }
                    }
                } else {
                    self.next_is_separator = true;
                    KIteratorNext::Output(output)
                }
            }
            other => other,
        }
    }
}

impl Iterator for IntersperseWith {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.peeked.take().or_else(|| self.iter.next());

        if next.is_some() {
            let result = if self.next_is_separator {
                self.peeked = next;
                let mut task = match self
                    .vm
                    .call_function_as_task(self.separator_function.clone(), &[])
                {
                    Ok(task) => task,
                    Err(mut error) => {
                        error.extend_trace(self.error_frame.clone());
                        return Some(Output::Error(error));
                    }
                };

                Some(
                    match poll_task_value_sync(&self.vm, &mut task, &self.error_frame) {
                        TaskValuePoll::Ready(result) => Output::Value(result),
                        TaskValuePoll::Pending => unreachable!(),
                        TaskValuePoll::Error(error) => error,
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
    error_frame: InstructionFrame,
    pending_predicate: Option<(Output, KTask)>,
}

impl Keep {
    /// Creates a new [Keep] adaptor
    pub fn new(iter: KIterator, predicate: KValue, vm: &KotoVm) -> Self {
        Self {
            iter,
            predicate,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            pending_predicate: None,
        }
    }
}

impl KotoIterator for Keep {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
            error_frame: self.error_frame.clone(),
            pending_predicate: self.pending_predicate.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        loop {
            if let Some((iter_output, mut task)) = self.pending_predicate.take() {
                match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                    TaskValuePoll::Ready(result) => {
                        match bool_result_to_next(result, iter_output, &self.error_frame) {
                            BoolNextResult::True(output) => {
                                return KIteratorNext::Output(output);
                            }
                            BoolNextResult::False => continue,
                            BoolNextResult::Error(error) => {
                                return KIteratorNext::Output(error);
                            }
                        }
                    }
                    TaskValuePoll::Pending => {
                        self.pending_predicate = Some((iter_output, task));
                        return KIteratorNext::Pending;
                    }
                    TaskValuePoll::Error(error) => return KIteratorNext::Output(error),
                }
            }

            let iter_output = match self.iter.next_output_with_context(context) {
                KIteratorNext::Output(Output::Error(error)) => {
                    return KIteratorNext::Output(Output::Error(error));
                }
                KIteratorNext::Output(output) => output,
                other => return other,
            };

            match call_function_with_output_as_task(
                &mut self.vm,
                self.predicate.clone(),
                &iter_output,
            ) {
                Ok(mut task) => {
                    match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                        TaskValuePoll::Ready(result) => {
                            match bool_result_to_next(result, iter_output, &self.error_frame) {
                                BoolNextResult::True(output) => {
                                    return KIteratorNext::Output(output);
                                }
                                BoolNextResult::False => continue,
                                BoolNextResult::Error(error) => {
                                    return KIteratorNext::Output(error);
                                }
                            }
                        }
                        TaskValuePoll::Pending => {
                            self.pending_predicate = Some((iter_output, task));
                            return KIteratorNext::Pending;
                        }
                        TaskValuePoll::Error(error) => return KIteratorNext::Output(error),
                    }
                }
                Err(mut error) => {
                    error.extend_trace(self.error_frame.clone());
                    return KIteratorNext::Output(Output::Error(error));
                }
            }
        }
    }
}

impl Iterator for Keep {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        for output in &mut self.iter {
            if matches!(output, Output::Error(_)) {
                return Some(output);
            }

            let mut task = match call_function_with_output_as_task(
                &mut self.vm,
                self.predicate.clone(),
                &output,
            ) {
                Ok(task) => task,
                Err(mut error) => {
                    error.extend_trace(self.error_frame.clone());
                    return Some(Output::Error(error));
                }
            };

            let result = match poll_task_value_sync(&self.vm, &mut task, &self.error_frame) {
                TaskValuePoll::Ready(KValue::Bool(false)) => continue,
                TaskValuePoll::Ready(KValue::Bool(true)) => output,
                TaskValuePoll::Ready(unexpected) => {
                    let error = Error::with_error_frame(
                        ErrorKind::UnexpectedType {
                            expected: "Bool from the predicate".into(),
                            unexpected,
                        },
                        self.error_frame.clone(),
                    );
                    Output::Error(error)
                }
                TaskValuePoll::Pending => unreachable!(),
                TaskValuePoll::Error(error) => error,
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        match self.iter.next_output_with_context(context) {
            KIteratorNext::Output(Output::ValuePair(first, _)) => {
                KIteratorNext::Output(Output::Value(first))
            }
            other => other,
        }
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        match self.iter.next_output_with_context(context) {
            KIteratorNext::Output(Output::ValuePair(_, second)) => {
                KIteratorNext::Output(Output::Value(second))
            }
            other => other,
        }
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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        self.iter.next_back_output_with_context(context)
    }

    fn next_back_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        self.iter.next_output_with_context(context)
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

/// An iterator that skips N values from the adapted iterator before yielding all following values
pub struct Skip {
    iter: KIterator,
    remaining: usize,
}

impl Skip {
    /// Creates a new [Skip] adaptor
    pub fn new(iter: KIterator, count: usize) -> Self {
        Self {
            iter,
            remaining: count,
        }
    }
}

impl KotoIterator for Skip {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            remaining: self.remaining,
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        while self.remaining > 0 {
            match self.iter.next_output_with_context(context) {
                KIteratorNext::Output(_) => self.remaining -= 1,
                KIteratorNext::Pending => return KIteratorNext::Pending,
                KIteratorNext::Done => {
                    self.remaining = 0;
                    return KIteratorNext::Done;
                }
            }
        }

        self.iter.next_output_with_context(context)
    }

    fn is_bidirectional(&self) -> bool {
        self.iter.is_bidirectional()
    }

    fn next_back(&mut self) -> Option<Output> {
        // Ensure the forward output has been skipped before yielding output from the back
        if self.remaining > 0 {
            self.iter.nth(self.remaining - 1);
            self.remaining = 0;
        }

        self.iter.next_back()
    }

    fn next_back_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        while self.remaining > 0 {
            match self.iter.next_output_with_context(context) {
                KIteratorNext::Output(_) => self.remaining -= 1,
                KIteratorNext::Pending => return KIteratorNext::Pending,
                KIteratorNext::Done => {
                    self.remaining = 0;
                    return KIteratorNext::Done;
                }
            }
        }

        self.iter.next_back_output_with_context(context)
    }
}

impl Iterator for Skip {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.iter.nth(take(&mut self.remaining))
        } else {
            self.iter.next()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (
            lower.saturating_sub(self.remaining),
            upper.map(|upper| upper.saturating_sub(self.remaining)),
        )
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
    pending_output: Option<Output>,
    remaining_skip: u64,
}

impl Step {
    /// Creates a new [Step] adaptor
    pub fn new(iter: KIterator, step: u64) -> StdResult<Self, StepError> {
        if step == 0 {
            Err(StepError::StepCantBeZero)
        } else {
            Ok(Self {
                iter,
                step,
                pending_output: None,
                remaining_skip: 0,
            })
        }
    }
}

impl KotoIterator for Step {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            step: self.step,
            pending_output: self.pending_output.clone(),
            remaining_skip: self.remaining_skip,
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        loop {
            if self.pending_output.is_none() {
                match self.iter.next_output_with_context(context) {
                    KIteratorNext::Output(output) => {
                        self.pending_output = Some(output);
                        self.remaining_skip = self.step - 1;
                    }
                    other => return other,
                }
            }

            while self.remaining_skip > 0 {
                match self.iter.next_output_with_context(context) {
                    KIteratorNext::Output(_) => self.remaining_skip -= 1,
                    KIteratorNext::Pending => return KIteratorNext::Pending,
                    KIteratorNext::Done => {
                        self.remaining_skip = 0;
                        break;
                    }
                }
            }

            if let Some(output) = self.pending_output.take() {
                return KIteratorNext::Output(output);
            }
        }
    }
}

impl Iterator for Step {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(output) = self.pending_output.take() {
            while self.remaining_skip > 0 {
                if self.iter.next().is_some() {
                    self.remaining_skip -= 1;
                } else {
                    self.remaining_skip = 0;
                    break;
                }
            }

            return Some(output);
        }

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

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if self.remaining > 0 {
            match self.iter.next_output_with_context(context) {
                output @ KIteratorNext::Output(_) => {
                    self.remaining -= 1;
                    output
                }
                KIteratorNext::Pending => KIteratorNext::Pending,
                KIteratorNext::Done => KIteratorNext::Done,
            }
        } else {
            KIteratorNext::Done
        }
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
    error_frame: InstructionFrame,
    finished: bool,
    pending_predicate: Option<(Output, KTask)>,
}

impl TakeWhile {
    /// Creates a new [Keep] adaptor
    pub fn new(iter: KIterator, predicate: KValue, vm: &KotoVm) -> Self {
        Self {
            iter,
            predicate,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            finished: false,
            pending_predicate: None,
        }
    }
}

impl KotoIterator for TakeWhile {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter: self.iter.make_copy()?,
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
            error_frame: self.error_frame.clone(),
            finished: self.finished,
            pending_predicate: self.pending_predicate.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if self.finished {
            return KIteratorNext::Done;
        }

        if let Some((iter_output, mut task)) = self.pending_predicate.take() {
            match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                TaskValuePoll::Ready(result) => {
                    match bool_result_to_next(result, iter_output, &self.error_frame) {
                        BoolNextResult::True(output) => {
                            return KIteratorNext::Output(output);
                        }
                        BoolNextResult::False => {
                            self.finished = true;
                            return KIteratorNext::Done;
                        }
                        BoolNextResult::Error(error) => {
                            return KIteratorNext::Output(error);
                        }
                    }
                }
                TaskValuePoll::Pending => {
                    self.pending_predicate = Some((iter_output, task));
                    return KIteratorNext::Pending;
                }
                TaskValuePoll::Error(error) => return KIteratorNext::Output(error),
            }
        }

        let iter_output = match self.iter.next_output_with_context(context) {
            KIteratorNext::Output(Output::Error(error)) => {
                return KIteratorNext::Output(Output::Error(error));
            }
            KIteratorNext::Output(output) => output,
            other => return other,
        };

        match call_function_with_output_as_task(&mut self.vm, self.predicate.clone(), &iter_output)
        {
            Ok(mut task) => {
                match poll_task_value(&self.vm, &mut task, context, &self.error_frame) {
                    TaskValuePoll::Ready(result) => {
                        match bool_result_to_next(result, iter_output, &self.error_frame) {
                            BoolNextResult::True(output) => KIteratorNext::Output(output),
                            BoolNextResult::False => {
                                self.finished = true;
                                KIteratorNext::Done
                            }
                            BoolNextResult::Error(error) => KIteratorNext::Output(error),
                        }
                    }
                    TaskValuePoll::Pending => {
                        self.pending_predicate = Some((iter_output, task));
                        KIteratorNext::Pending
                    }
                    TaskValuePoll::Error(error) => KIteratorNext::Output(error),
                }
            }
            Err(mut error) => {
                error.extend_trace(self.error_frame.clone());
                KIteratorNext::Output(Output::Error(error))
            }
        }
    }
}

impl Iterator for TakeWhile {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let iter_output = self.iter.next()?;
        if matches!(iter_output, Output::Error(_)) {
            return Some(iter_output);
        }

        let mut task = match call_function_with_output_as_task(
            &mut self.vm,
            self.predicate.clone(),
            &iter_output,
        ) {
            Ok(task) => task,
            Err(mut error) => {
                error.extend_trace(self.error_frame.clone());
                return Some(Output::Error(error));
            }
        };

        let result = match poll_task_value_sync(&self.vm, &mut task, &self.error_frame) {
            TaskValuePoll::Ready(KValue::Bool(true)) => iter_output,
            TaskValuePoll::Ready(KValue::Bool(false)) => {
                self.finished = true;
                return None;
            }
            TaskValuePoll::Ready(unexpected) => {
                let error = Error::with_error_frame(
                    ErrorKind::UnexpectedType {
                        expected: "Bool from the predicate".into(),
                        unexpected,
                    },
                    self.error_frame.clone(),
                );
                Output::Error(error)
            }
            TaskValuePoll::Pending => unreachable!(),
            TaskValuePoll::Error(error) => error,
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
    pop_front_on_next: bool,
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
                pop_front_on_next: false,
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
            pop_front_on_next: self.pop_front_on_next,
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if self.pop_front_on_next {
            self.cache.pop_front();
            self.pop_front_on_next = false;
        }

        while self.cache.len() < self.window_size {
            match self.iter.next_output_with_context(context) {
                KIteratorNext::Output(output) => match KValue::try_from(output) {
                    Ok(value) => self.cache.push_back(value),
                    Err(error) => return KIteratorNext::Output(Output::Error(error)),
                },
                KIteratorNext::Pending => return KIteratorNext::Pending,
                KIteratorNext::Done => break,
            }
        }

        if self.cache.len() == self.window_size {
            let result: Vec<_> = self.cache.iter().cloned().collect();
            self.pop_front_on_next = true;
            KIteratorNext::Output(KTuple::from(result).into())
        } else {
            KIteratorNext::Done
        }
    }
}

impl Iterator for Windows {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pop_front_on_next {
            self.cache.pop_front();
            self.pop_front_on_next = false;
        }

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
            self.pop_front_on_next = true;
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
    pending_a: Option<KValue>,
}

impl Zip {
    /// Creates a new [Zip] adaptor
    pub fn new(iter_a: KIterator, iter_b: KIterator) -> Self {
        Self {
            iter_a,
            iter_b,
            pending_a: None,
        }
    }
}

impl KotoIterator for Zip {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            iter_a: self.iter_a.make_copy()?,
            iter_b: self.iter_b.make_copy()?,
            pending_a: self.pending_a.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        if self.pending_a.is_none() {
            match self
                .iter_a
                .next_output_with_context(context)
                .map(collect_pair)
            {
                KIteratorNext::Output(Output::Value(value_a)) => self.pending_a = Some(value_a),
                KIteratorNext::Output(Output::Error(error)) => {
                    return KIteratorNext::Output(Output::Error(error));
                }
                KIteratorNext::Output(_) => unreachable!(),
                KIteratorNext::Pending => return KIteratorNext::Pending,
                KIteratorNext::Done => return KIteratorNext::Done,
            }
        }

        match self
            .iter_b
            .next_output_with_context(context)
            .map(collect_pair)
        {
            KIteratorNext::Output(Output::Value(value_b)) => {
                let value_a = self.pending_a.take().unwrap();
                KIteratorNext::Output(Output::ValuePair(value_a, value_b))
            }
            KIteratorNext::Output(Output::Error(error)) => {
                KIteratorNext::Output(Output::Error(error))
            }
            KIteratorNext::Output(_) => unreachable!(),
            KIteratorNext::Pending => KIteratorNext::Pending,
            KIteratorNext::Done => {
                self.pending_a = None;
                KIteratorNext::Done
            }
        }
    }
}

impl Iterator for Zip {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let value_a = match self.pending_a.take() {
            Some(value_a) => value_a,
            None => match self.iter_a.next().map(collect_pair) {
                Some(Output::Value(value_a)) => value_a,
                error @ Some(Output::Error(_)) => return error,
                _ => return None,
            },
        };

        match self.iter_b.next().map(collect_pair) {
            Some(Output::Value(value_b)) => Some(Output::ValuePair(value_a, value_b)),
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
