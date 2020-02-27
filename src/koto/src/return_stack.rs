use crate::Value;
use vec1::Vec1;

#[derive(Debug)]
pub struct ReturnStack<'a> {
    values: Vec<Value<'a>>,
    frame_size: Vec1<usize>,
}

impl<'a> ReturnStack<'a>{
    pub fn new() -> Self {
        let initial_capacity = 32;
        Self {
            values: Vec::with_capacity(initial_capacity),
            frame_size: Vec1::with_capacity(initial_capacity, 0),
        }
    }

    #[allow(dead_code)]
    pub fn frame_count(&self) -> usize {
        self.frame_size.len()
    }

    pub fn push(&mut self, value: Value<'a>) {
        self.values.push(value);
        *self.frame_size.last_mut() += 1;
    }

    pub fn start_frame(&mut self) {
        self.frame_size.push(0);
    }

    pub fn pop_frame(&mut self) {
        if let Ok(value_count) = self.frame_size.try_pop() {
            for _ in 0..value_count {
                self.values.pop();
            }
        }
    }

    pub fn pop_frame_and_keep_results(&mut self) {
        if let Ok(value_count) = self.frame_size.try_pop() {
            *self.frame_size.last_mut() += value_count;
        }
    }

    pub fn value(&self) -> &Value<'a> {
        let values_start = self.values.len() - self.value_count();
        &self.values[values_start]
    }

    pub fn values(&self) -> &[Value<'a>] {
        let values_start = self.values.len() - self.value_count();
        &self.values[values_start..]
    }

    pub fn value_count(&self) -> usize {
        *self.frame_size.last()
    }
}
