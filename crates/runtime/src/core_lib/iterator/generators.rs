//! Generators used by the `iterator` core library module

use crate::{prelude::*, KIteratorOutput as Output, Result};

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
}

impl Generate {
    /// Creates a new [Generate] generator
    pub fn new(function: KValue, vm: KotoVm) -> Self {
        Self { function, vm }
    }
}

impl KotoIterator for Generate {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for Generate {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let function = self.function.clone();
        let result = match self.vm.call_function(function, &[]) {
            Ok(result) => Output::Value(result),
            Err(error) => Output::Error(error),
        };
        Some(result)
    }
}

/// An iterator that yields the result of calling a function N times
pub struct GenerateN {
    remaining: usize,
    function: KValue,
    vm: KotoVm,
}

impl GenerateN {
    /// Creates a new [GenerateN] generator
    pub fn new(n: usize, function: KValue, vm: KotoVm) -> Self {
        Self {
            remaining: n,
            function,
            vm,
        }
    }
}

impl KotoIterator for GenerateN {
    fn make_copy(&self) -> Result<KIterator> {
        let result = Self {
            remaining: self.remaining,
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        Ok(KIterator::new(result))
    }
}

impl Iterator for GenerateN {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            let function = self.function.clone();
            let result = match self.vm.call_function(function, &[]) {
                Ok(result) => Output::Value(result),
                Err(error) => Output::Error(error),
            };
            Some(result)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
