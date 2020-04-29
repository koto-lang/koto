use crate::{Value, ValueList};

#[derive(Clone, Copy, Debug)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

#[derive(Clone, Debug)]
pub enum Iterable {
    Range(IntRange),
    List(ValueList),
}

#[derive(Clone, Debug)]
pub struct ValueIterator {
    index: usize,
    iterable: Iterable,
}

impl ValueIterator {
    pub fn new(iterable: Iterable) -> Self {
        Self { index: 0, iterable }
    }
}

impl Iterator for ValueIterator {
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
            Iterable::List(list) => {
                let result = list.data().get(self.index).cloned();
                self.index += 1;
                result
            }
        }
    }
}
