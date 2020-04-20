use crate::{Value, ValueVec};
use smallvec::SmallVec;

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
            Range { start, end } => {
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
