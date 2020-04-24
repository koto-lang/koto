use crate::{Value, ValueVec};
use smallvec::SmallVec;

#[derive(Clone, Copy, Debug)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

#[derive(Clone, Debug)]
pub enum Iterable {
    Range(IntRange),
}

#[derive(Clone, Debug)]
pub struct ValueIterator2 {
    index: usize,
    iterable: Iterable,
}

impl ValueIterator2 {
    pub fn new(iterable: Iterable) -> Self {
        Self { index: 0, iterable }
    }
}

impl Iterator for ValueIterator2 {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        use Value::Number;

        match &self.iterable {
            Iterable::Range(IntRange { start, end }) => {
                if start <= end {
                    // ascending range
                    let result = start + self.index as isize;
                    if result < *end {
                        self.index += 1;
                        Some(Number(result as f64))
                    } else {
                        None
                    }
                } else {
                    // descending range
                    let result = start - self.index as isize - 1; // TODO avoid -1
                    if result >= *end {
                        self.index += 1;
                        Some(Number(result as f64))
                    } else {
                        None
                    }
                }
            }
        }
    }
}

// ----------
// ----------

pub(super) struct ValueIterator {
    value: Value,
    index: isize,
}

impl ValueIterator {
    pub fn new(value: Value) -> Self {
        Self { value, index: 0 }
    }
}

impl Iterator for ValueIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        use Value::*;

        let result = match &self.value {
            List(l) => l.data().get(self.index as usize).cloned(),
            Range(IntRange { start, end }) => {
                if start <= end {
                    if self.index < (end - start) {
                        Some(Number((start + self.index) as f64))
                    } else {
                        None
                    }
                } else if self.index < (start - end) {
                    Some(Number((start - self.index - 1) as f64))
                } else {
                    None
                }
            }
            _ => None,
        };

        if result.is_some() {
            self.index += 1;
        }

        result
    }
}

pub(super) struct MultiRangeValueIterator {
    pub iterators: SmallVec<[ValueIterator; 4]>,
}

impl MultiRangeValueIterator {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            iterators: SmallVec::with_capacity(capacity),
        }
    }

    pub fn get_next_values(&mut self, output: &mut ValueVec) -> bool {
        output.clear();

        for iter in self.iterators.iter_mut() {
            match iter.next() {
                Some(value) => output.push(value.clone()),
                None => {
                    return false;
                }
            }
        }

        true
    }
}
