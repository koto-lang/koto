//! Generators used by the `iterator` core library module

use crate::{InstructionFrame, KIteratorNext, KIteratorOutput as Output, Result, prelude::*};
use std::task::Context;

fn poll_task(
    _vm: &KotoVm,
    task: &mut KTask,
    context: &mut Context<'_>,
    error_frame: &InstructionFrame,
) -> KIteratorNext {
    loop {
        match task.poll_with_context(context) {
            Ok(KTaskPoll::Ready(KValue::Task(nested))) => *task = nested,
            Ok(KTaskPoll::Ready(result)) => return KIteratorNext::Output(Output::Value(result)),
            Ok(KTaskPoll::Pending) => return KIteratorNext::Pending,
            Err(mut error) => {
                error.extend_trace(error_frame.clone());
                return KIteratorNext::Output(Output::Error(error));
            }
        }
    }
}

fn poll_task_to_output_sync(
    vm: &KotoVm,
    task: &mut KTask,
    error_frame: &InstructionFrame,
) -> Output {
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);

    loop {
        match poll_task(vm, task, &mut context, error_frame) {
            KIteratorNext::Output(output) => return output,
            KIteratorNext::Pending => std::thread::yield_now(),
            KIteratorNext::Done => unreachable!(),
        }
    }
}

fn call_function_to_output_sync(
    vm: &mut KotoVm,
    function: KValue,
    error_frame: &InstructionFrame,
) -> Output {
    let mut task = match vm.call_function_as_task(function, &[]) {
        Ok(task) => task,
        Err(mut error) => {
            error.extend_trace(error_frame.clone());
            return Output::Error(error);
        }
    };

    poll_task_to_output_sync(vm, &mut task, error_frame)
}

/// An iterator that yields a value once
#[derive(Clone)]
pub struct Once {
    value: Option<KValue>,
}

impl Once {
    /// Creates a new [Once] generator
    pub fn new(value: KValue) -> Self {
        Self { value: Some(value) }
    }
}

impl KotoIterator for Once {
    fn make_copy(&self) -> Result<KIterator> {
        Ok(KIterator::new(self.clone()))
    }
}

impl Iterator for Once {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.value.take().map(Output::Value)
    }
}

/// An iterator that repeatedly yields the same value
pub struct Repeat {
    value: KValue,
}

impl Repeat {
    /// Creates a new [Repeat] generator
    pub fn new(value: KValue) -> Self {
        Self { value }
    }
}

impl KotoIterator for Repeat {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            value: self.value.clone(),
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Repeat {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Output::Value(self.value.clone()))
    }
}

/// An iterator that yields the same value N times
pub struct RepeatN {
    remaining: usize,
    value: KValue,
}

impl RepeatN {
    /// Creates a new [RepeatN] generator
    pub fn new(value: KValue, n: usize) -> Self {
        Self {
            remaining: n,
            value,
        }
    }
}

impl KotoIterator for RepeatN {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            remaining: self.remaining,
            value: self.value.clone(),
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for RepeatN {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            Some(Output::Value(self.value.clone()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

/// An iterator that repeatedly yields the result of calling a function
pub struct Generate {
    function: KValue,
    vm: KotoVm,
    error_frame: InstructionFrame,
    pending_function: Option<KTask>,
}

impl Generate {
    /// Creates a new [Generate] generator
    pub fn new(function: KValue, vm: &KotoVm) -> Self {
        Self {
            function,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            pending_function: None,
        }
    }
}

impl KotoIterator for Generate {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
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

        match self.vm.call_function_as_task(self.function.clone(), &[]) {
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

impl Iterator for Generate {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        Some(call_function_to_output_sync(
            &mut self.vm,
            self.function.clone(),
            &self.error_frame,
        ))
    }
}

/// An iterator that yields the result of calling a function N times
pub struct GenerateN {
    remaining: usize,
    function: KValue,
    vm: KotoVm,
    error_frame: InstructionFrame,
    pending_function: Option<KTask>,
}

impl GenerateN {
    /// Creates a new [GenerateN] generator
    pub fn new(n: usize, function: KValue, vm: &KotoVm) -> Self {
        Self {
            remaining: n,
            function,
            vm: vm.spawn_shared_vm(),
            error_frame: vm.instruction_frame(),
            pending_function: None,
        }
    }
}

impl KotoIterator for GenerateN {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            remaining: self.remaining,
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

        if self.remaining > 0 {
            self.remaining -= 1;

            match self.vm.call_function_as_task(self.function.clone(), &[]) {
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
        } else {
            KIteratorNext::Done
        }
    }
}

impl Iterator for GenerateN {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            Some(call_function_to_output_sync(
                &mut self.vm,
                self.function.clone(),
                &self.error_frame,
            ))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
