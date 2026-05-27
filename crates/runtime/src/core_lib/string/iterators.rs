//! A collection of string iterators

use crate::{
    Error, ErrorKind, InstructionFrame, KIteratorNext, KIteratorOutput as Output, Result,
    prelude::*,
};
use std::task::Context;
use unicode_segmentation::UnicodeSegmentation;

/// An iterator that outputs the individual bytes contained in a string
#[derive(Clone)]
pub struct Bytes {
    input: KString,
    index: usize,
}

impl Bytes {
    /// Creates a new [Bytes] iterator
    pub fn new(input: KString) -> Self {
        Self { input, index: 0 }
    }
}

impl KotoIterator for Bytes {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl Iterator for Bytes {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.input.as_bytes().get(self.index) {
            Some(byte) => {
                self.index += 1;
                Some(Output::Value(byte.into()))
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.input.len() - self.index;
        (remaining, Some(remaining))
    }
}

/// An iterator that outputs the individual bytes contained in a string
#[derive(Clone)]
pub struct CharIndices {
    input: KString,
    index: usize,
}

impl CharIndices {
    /// Creates a new [CharIndices] iterator
    pub fn new(input: KString) -> Self {
        Self { input, index: 0 }
    }
}

impl KotoIterator for CharIndices {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl Iterator for CharIndices {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.input[self.index..]
            .grapheme_indices(true)
            .next()
            .map(|(start, grapheme)| {
                let start = self.index + start;
                let end = start + grapheme.len();
                self.index += grapheme.len();
                let result = KRange::from(start as i64..end as i64);
                Output::Value(result.into())
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.input.len() - self.index;
        (remaining, Some(remaining))
    }
}

/// An iterator that yields the lines contained in a string
///
/// - Lines end with either `\r\n` or `\n`.
/// - Line end characters aren't included in the resulting output.
/// - Empty lines are yielded as empty strings.
#[derive(Clone)]
pub struct Lines {
    input: KString,
    start: usize,
}

impl Lines {
    /// Creates a new [Lines] iterator
    pub fn new(input: KString) -> Self {
        Self { input, start: 0 }
    }
}

impl KotoIterator for Lines {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl Iterator for Lines {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.start;
        if start < self.input.len() {
            let mut newline_bytes = 1;
            let remaining = &self.input[start..];

            let end = match remaining.find('\n') {
                Some(end) => {
                    if end > 0 && remaining.as_bytes()[end - 1] == b'\r' {
                        newline_bytes += 1;
                        start + end - 1
                    } else {
                        start + end
                    }
                }
                None => self.input.len(),
            };

            let result = KValue::Str(self.input.with_bounds(start..end).unwrap());
            self.start = end + newline_bytes;
            Some(Output::Value(result))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_bytes = self.input.len() - self.start;
        (1.min(remaining_bytes), Some(remaining_bytes))
    }
}

/// An iterator that splits up a string into parts, separated by a provided pattern
#[derive(Clone)]
pub struct Split {
    input: KString,
    pattern: KString,
    start: usize,
}

impl Split {
    /// Creates a new [Split] iterator
    pub fn new(input: KString, pattern: KString) -> Self {
        Self {
            input,
            pattern,
            start: 0,
        }
    }
}

impl KotoIterator for Split {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl Iterator for Split {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.start;
        if start <= self.input.len() {
            let end = match self.input[start..].find(self.pattern.as_str()) {
                Some(end) => start + end,
                None => self.input.len(),
            };

            let output = KValue::Str(self.input.with_bounds(start..end).unwrap());
            self.start = end + self.pattern.len();
            Some(Output::Value(output))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_bytes = self.input.len() - self.start;
        (1.min(remaining_bytes), Some(remaining_bytes))
    }
}

/// An iterator that splits up a string into parts, separated when a char passes a predicate
pub struct SplitWith {
    input: KString,
    predicate: KValue,
    vm: KotoVm,
    error_frame: InstructionFrame,
    start: usize,
    scan_index: usize,
    pending_predicate: Option<PendingSplitPredicate>,
}

impl SplitWith {
    /// Creates a new [SplitWith] iterator
    pub fn new(input: KString, predicate: KValue, vm: &KotoVm) -> Self {
        Self {
            input,
            predicate,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            start: 0,
            scan_index: 0,
            pending_predicate: None,
        }
    }

    fn handle_predicate_result(
        &mut self,
        result: KValue,
        grapheme_start: usize,
        grapheme_len: usize,
    ) -> SplitPredicateResult {
        use KValue::{Bool, Str};

        match result {
            Bool(true) => {
                let output = Str(self.input.with_bounds(self.start..grapheme_start).unwrap());
                self.start = grapheme_start + grapheme_len;
                self.scan_index = self.start;
                SplitPredicateResult::Output(Output::Value(output))
            }
            Bool(false) => {
                self.scan_index = grapheme_start + grapheme_len;
                SplitPredicateResult::Continue
            }
            unexpected => {
                let error = Error::with_error_frame(
                    ErrorKind::UnexpectedType {
                        expected: "Bool from the match function".into(),
                        unexpected,
                    },
                    self.error_frame.clone(),
                );
                SplitPredicateResult::Output(Output::Error(error))
            }
        }
    }

    fn poll_predicate_task(
        &self,
        mut task: KTask,
        context: &mut Context<'_>,
    ) -> SplitPredicateTaskPoll {
        loop {
            match task.poll_with_context(context) {
                Ok(KTaskPoll::Ready(KValue::Task(nested))) => task = nested,
                Ok(KTaskPoll::Ready(result)) => return SplitPredicateTaskPoll::Ready(result),
                Ok(KTaskPoll::Pending) => return SplitPredicateTaskPoll::Pending(task),
                Err(mut error) => {
                    error.extend_trace(self.error_frame.clone());
                    return SplitPredicateTaskPoll::Error(error);
                }
            }
        }
    }

    fn poll_predicate_task_sync(&self, task: KTask) -> SplitPredicateTaskPoll {
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut task = task;

        loop {
            match self.poll_predicate_task(task, &mut context) {
                SplitPredicateTaskPoll::Pending(pending) => {
                    task = pending;
                    std::thread::yield_now();
                }
                result => return result,
            }
        }
    }
}

impl KotoIterator for SplitWith {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            input: self.input.clone(),
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
            error_frame: self.error_frame.clone(),
            start: self.start,
            scan_index: self.scan_index,
            pending_predicate: self.pending_predicate.clone(),
        };
        Ok(KIterator::new(result))
    }

    fn next_output_with_context(&mut self, context: &mut Context<'_>) -> KIteratorNext {
        use KValue::Str;

        if self.start >= self.input.len() {
            return KIteratorNext::Done;
        }

        loop {
            if let Some(mut pending) = self.pending_predicate.take() {
                match self.poll_predicate_task(pending.task, context) {
                    SplitPredicateTaskPoll::Ready(result) => {
                        match self.handle_predicate_result(
                            result,
                            pending.grapheme_start,
                            pending.grapheme_len,
                        ) {
                            SplitPredicateResult::Continue => continue,
                            SplitPredicateResult::Output(output) => {
                                return KIteratorNext::Output(output);
                            }
                        }
                    }
                    SplitPredicateTaskPoll::Pending(task) => {
                        pending.task = task;
                        self.pending_predicate = Some(pending);
                        return KIteratorNext::Pending;
                    }
                    SplitPredicateTaskPoll::Error(error) => {
                        return KIteratorNext::Output(Output::Error(error));
                    }
                }
            }

            if self.scan_index >= self.input.len() {
                let output = Str(self
                    .input
                    .with_bounds(self.start..self.input.len())
                    .unwrap());
                self.start = self.input.len();
                self.scan_index = self.start;
                return KIteratorNext::Output(Output::Value(output));
            }

            let Some((grapheme_index, grapheme)) =
                self.input[self.scan_index..].grapheme_indices(true).next()
            else {
                return KIteratorNext::Done;
            };
            let grapheme_len = grapheme.len();
            let grapheme_start = self.scan_index + grapheme_index;
            let grapheme_end = grapheme_start + grapheme_len;
            let x = self
                .input
                .with_bounds(grapheme_start..grapheme_end)
                .unwrap();

            match self.vm.call_function_as_task(self.predicate.clone(), x) {
                Ok(task) => match self.poll_predicate_task(task, context) {
                    SplitPredicateTaskPoll::Ready(result) => {
                        match self.handle_predicate_result(result, grapheme_start, grapheme_len) {
                            SplitPredicateResult::Continue => continue,
                            SplitPredicateResult::Output(output) => {
                                return KIteratorNext::Output(output);
                            }
                        }
                    }
                    SplitPredicateTaskPoll::Pending(task) => {
                        self.pending_predicate = Some(PendingSplitPredicate {
                            grapheme_start,
                            grapheme_len,
                            task,
                        });
                        return KIteratorNext::Pending;
                    }
                    SplitPredicateTaskPoll::Error(error) => {
                        return KIteratorNext::Output(Output::Error(error));
                    }
                },
                Err(mut error) => {
                    error.extend_trace(self.error_frame.clone());
                    return KIteratorNext::Output(Output::Error(error));
                }
            }
        }
    }
}

#[derive(Clone)]
struct PendingSplitPredicate {
    grapheme_start: usize,
    grapheme_len: usize,
    task: KTask,
}

enum SplitPredicateTaskPoll {
    Ready(KValue),
    Pending(KTask),
    Error(Error),
}

enum SplitPredicateResult {
    Output(Output),
    Continue,
}

impl Iterator for SplitWith {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        use KValue::{Bool, Str};

        let start = self.start;
        if start < self.input.len() {
            let mut end = None;
            let mut grapheme_len = 0;

            for (grapheme_index, grapheme) in self.input[start..].grapheme_indices(true) {
                grapheme_len = grapheme.len();
                let grapheme_start = start + grapheme_index;
                let grapheme_end = grapheme_start + grapheme_len;
                let x = self
                    .input
                    .with_bounds(grapheme_start..grapheme_end)
                    .unwrap();
                let task = match self.vm.call_function_as_task(self.predicate.clone(), x) {
                    Ok(task) => task,
                    Err(mut error) => {
                        error.extend_trace(self.error_frame.clone());
                        return Some(Output::Error(error));
                    }
                };

                match self.poll_predicate_task_sync(task) {
                    SplitPredicateTaskPoll::Ready(Bool(split_match)) => {
                        if split_match {
                            end = Some(grapheme_start);
                            break;
                        }
                    }
                    SplitPredicateTaskPoll::Ready(unexpected) => {
                        let error = Error::with_error_frame(
                            ErrorKind::UnexpectedType {
                                expected: "Bool from the match function".into(),
                                unexpected,
                            },
                            self.error_frame.clone(),
                        );
                        return Some(Output::Error(error));
                    }
                    SplitPredicateTaskPoll::Pending(_) => unreachable!(),
                    SplitPredicateTaskPoll::Error(error) => return Some(Output::Error(error)),
                }
            }

            let end = end.unwrap_or(self.input.len());
            let output = Str(self.input.with_bounds(start..end).unwrap());
            self.start = end + grapheme_len;
            self.scan_index = self.start;

            Some(Output::Value(output))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_bytes = self.input.len() - self.start;
        (1.min(remaining_bytes), Some(remaining_bytes))
    }
}
