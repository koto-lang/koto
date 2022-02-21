use crate::{
    value_iterator::{KotoIterator, ValueIterator, ValueIteratorOutput as Output},
    Value,
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
    pub fn new(n: usize, value: Value) -> Self {
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
