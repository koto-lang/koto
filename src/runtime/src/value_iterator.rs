use crate::{Value, ValueVec};
use smallvec::SmallVec;

pub(super) struct ValueIterator<'a> {
    value: Value<'a>,
    index: isize,
}

impl<'a> ValueIterator<'a> {
    pub fn new(value: Value<'a>) -> Self {
        Self { value, index: 0 }
    }
}

impl<'a> Iterator for ValueIterator<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Value<'a>> {
        use Value::*;

        let result = match &self.value {
            List(l) => l.borrow().data().get(self.index as usize).cloned(),
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

pub(super) struct MultiRangeValueIterator<'a> {
    pub iterators: SmallVec<[ValueIterator<'a>; 4]>,
}

impl<'a> MultiRangeValueIterator<'a> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            iterators: SmallVec::with_capacity(capacity),
        }
    }

    pub fn get_next_values(&mut self, output: &mut ValueVec<'a>) -> bool {
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
