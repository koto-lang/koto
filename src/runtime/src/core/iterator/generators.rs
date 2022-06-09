use crate::{
    value_iterator::{KotoIterator, ValueIterator, ValueIteratorOutput as Output},
    CallArgs, Value, Vm,
};

/// An iterator that repeatedly yields the same value
pub struct Repeat {
    value: Value,
}

impl Repeat {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

impl KotoIterator for Repeat {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            value: self.value.clone(),
        };
        ValueIterator::new(result)
    }

    fn might_have_side_effects(&self) -> bool {
        false
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
    value: Value,
}

impl RepeatN {
    pub fn new(value: Value, n: usize) -> Self {
        Self {
            remaining: n,
            value,
        }
    }
}

impl KotoIterator for RepeatN {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            remaining: self.remaining,
            value: self.value.clone(),
        };
        ValueIterator::new(result)
    }

    fn might_have_side_effects(&self) -> bool {
        false
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
    function: Value,
    vm: Vm,
}

impl Generate {
    pub fn new(function: Value, vm: Vm) -> Self {
        Self { function, vm }
    }
}

impl KotoIterator for Generate {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::new(result)
    }

    fn might_have_side_effects(&self) -> bool {
        true
    }
}

impl Iterator for Generate {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let function = self.function.clone();
        let result = match self.vm.run_function(function, CallArgs::None) {
            Ok(result) => Output::Value(result),
            Err(error) => Output::Error(error),
        };
        Some(result)
    }
}

/// An iterator that yields the result of calling a function N times
pub struct GenerateN {
    remaining: usize,
    function: Value,
    vm: Vm,
}

impl GenerateN {
    pub fn new(n: usize, function: Value, vm: Vm) -> Self {
        Self {
            remaining: n,
            function,
            vm,
        }
    }
}

impl KotoIterator for GenerateN {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            remaining: self.remaining,
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::new(result)
    }

    fn might_have_side_effects(&self) -> bool {
        true
    }
}

impl Iterator for GenerateN {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            let function = self.function.clone();
            let result = match self.vm.run_function(function, CallArgs::None) {
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
