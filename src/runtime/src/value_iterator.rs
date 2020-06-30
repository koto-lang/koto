use crate::{Value, ValueList, ValueMap};

#[derive(Clone, Copy, Debug)]
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
        use Value::{List, Number, Str};

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
            Iterable::Map(map) => {
                let result = match map.data().get_index(self.index) {
                    // TODO - Introduce multivalue to avoid list creation
                    Some((key, value)) => Some(List(ValueList::from_slice(&[
                        Str(key.as_arc_string().clone()),
                        value.clone(),
                    ]))),
                    None => None,
                };

                self.index += 1;
                result
            }
        }
    }
}
