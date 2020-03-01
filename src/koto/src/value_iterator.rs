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
            List(a) => a.get(self.index as usize).cloned(),
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

pub(super) struct MultiRangeValueIterator<'a>(pub Vec<ValueIterator<'a>>);

impl<'a> MultiRangeValueIterator<'a> {
    pub fn push_next_values_to_stack(&mut self, value_stack: &mut ValueStack<'a>) -> bool {
        value_stack.start_frame();

        for iter in self.0.iter_mut() {
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
