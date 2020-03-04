use crate::{value_stack::ValueStack, Value};

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
            List(l) => l.data().get(self.index as usize).cloned(),
            Range { min, max } => {
                if self.index < (max - min) {
                    Some(Number((min + self.index) as f64))
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
    pub iterators: Vec<ValueIterator<'a>>,
}

impl<'a> MultiRangeValueIterator<'a> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            iterators: Vec::with_capacity(capacity),
        }
    }

    pub fn push_next_values_to_stack(&mut self, value_stack: &mut ValueStack<'a>) -> bool {
        value_stack.start_frame();

        for iter in self.iterators.iter_mut() {
            match iter.next() {
                Some(value) => value_stack.push(value.clone()),
                None => {
                    value_stack.pop_frame();
                    return false;
                }
            }
        }

        true
    }
}
