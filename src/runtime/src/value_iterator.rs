use crate::{Value, ValueList, ValueMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

#[derive(Clone, Debug)]
pub enum Iterable {
    Range(IntRange),
    List(ValueList),
    Map(ValueMap),
}

pub enum ValueIteratorOutput {
    Value(Value),
    ValuePair(Value, Value),
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
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        use Value::Number;

        match &self.iterable {
            Iterable::Range(IntRange { start, end }) => {
                if start <= end {
                    // ascending range
                    let result = start + self.index as isize;
                    if result < *end {
                        self.index += 1;
                        Some(ValueIteratorOutput::Value(Number(result as f64)))
                    } else {
                        None
                    }
                } else {
                    // descending range
                    let result = start - self.index as isize - 1; // TODO avoid -1
                    if result >= *end {
                        self.index += 1;
                        Some(ValueIteratorOutput::Value(Number(result as f64)))
                    } else {
                        None
                    }
                }
            }
            Iterable::List(list) => {
                let result = list
                    .data()
                    .get(self.index)
                    .map(|value| ValueIteratorOutput::Value(value.clone()));
                self.index += 1;
                result
            }
            Iterable::Map(map) => {
                let result = match map.data().get_index(self.index) {
                    Some((key, value)) => {
                        Some(ValueIteratorOutput::ValuePair(key.clone(), value.clone()))
                    }
                    None => None,
                };

                self.index += 1;
                result
            }
        }
    }
}
