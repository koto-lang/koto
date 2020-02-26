use crate::Value;

#[derive(Debug)]
pub struct ReturnStack {
    values: Vec<Value>,
    frame_size: Vec<usize>,
}

impl ReturnStack {
    pub fn new() -> Self {
        let initial_capacity = 32;
        Self {
            values: Vec::with_capacity(initial_capacity),
            frame_size: Vec::with_capacity(initial_capacity),
        }
    }

    #[allow(dead_code)]
    pub fn frame_count(&self) -> usize {
        self.frame_size.len()
    }

    pub fn push(&mut self, value: Value) {
        self.values.push(value);
        *self.frame_size.last_mut().unwrap() += 1;
    }

    pub fn start_frame(&mut self) {
        self.frame_size.push(0);
    }

    pub fn pop_frame(&mut self) {
        if let Some(value_count) = self.frame_size.pop() {
            for _ in 0..value_count {
                self.values.pop();
            }
        }
    }

    pub fn pop_frame_and_keep_results(&mut self) {
        // println!("pop_frame_and_keep_results");
        if let Some(value_count) = self.frame_size.pop() {
            *self.frame_size.last_mut().unwrap() += value_count;
        }
        //assert!(!self.frame_size.is_empty());
    }

    pub fn value(&self) -> &Value {
        let values_start = self.values.len() - self.value_count();
        &self.values[values_start]
    }

    pub fn values(&self) -> &[Value] {
        let values_start = self.values.len() - self.value_count();
        &self.values[values_start..]
    }

    pub fn value_count(&self) -> usize {
        match self.frame_size.last() {
            Some(size) => *size,
            None => unreachable!(),
        }
    }
}
