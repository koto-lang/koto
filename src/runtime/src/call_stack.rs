use crate::{Id, Value};

#[derive(Default)]
pub struct CallStack<'a> {
    values: Vec<(Id, Value<'a>)>,
    frame_size: Vec<usize>,
    pending_frame_size: usize,
}

impl<'a> CallStack<'a> {
    pub fn with_capacity(initial_capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(initial_capacity),
            frame_size: Vec::with_capacity(initial_capacity),
            pending_frame_size: 0,
        }
    }

    pub fn frame(&self) -> usize {
        self.frame_size.len()
    }

    pub fn push(&mut self, id: Id, value: Value<'a>) {
        self.values.push((id, value));
        self.pending_frame_size += 1;
    }

    pub fn extend(&mut self, id: Id, value: Value<'a>) {
        assert_eq!(
            self.pending_frame_size, 0,
            "Extend called before commit or cancel"
        );
        self.values.push((id, value));
        *self
            .frame_size
            .last_mut()
            .expect("Extend called before commiting a frame") += 1;
    }

    pub fn commit(&mut self) {
        self.frame_size.push(self.pending_frame_size);
        self.pending_frame_size = 0;
    }

    pub fn cancel(&mut self) {
        for _ in 0..self.pending_frame_size {
            self.values.pop();
        }
        self.pending_frame_size = 0;
    }

    pub fn pop_frame(&mut self) {
        if let Some(value_count) = self.frame_size.pop() {
            for _ in 0..value_count {
                self.values.pop();
            }
        }
    }

    pub fn frame_values(&self) -> Option<&[(Id, Value<'a>)]> {
        match self.frame_size.last() {
            Some(size) => {
                let values_start = self.values.len() - self.pending_frame_size - size;
                Some(&self.values[values_start..(values_start + size)])
            }
            None => None,
        }
    }

    fn frame_values_mut(&mut self) -> Option<&mut [(Id, Value<'a>)]> {
        match self.frame_size.last() {
            Some(size) => {
                let values_start = self.values.len() - self.pending_frame_size - size;
                Some(&mut self.values[values_start..(values_start + size)])
            }
            None => None,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Value<'a>> {
        match self.frame_values() {
            Some(values) => values.iter().find_map(
                |(value_id, value)| {
                    if value_id == id {
                        Some(value)
                    } else {
                        None
                    }
                },
            ),
            None => None,
        }
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Value<'a>> {
        match self.frame_values_mut() {
            Some(values) => {
                values.iter_mut().find_map(
                    |(value_id, value)| {
                        if value_id == id {
                            Some(value)
                        } else {
                            None
                        }
                    },
                )
            }
            None => None,
        }
    }

    pub fn make_unique(&mut self, id: &str) -> Option<Value<'a>> {
        if let Some(values) = self.frame_values_mut() {
            for (value_id, value) in values.iter_mut() {
                if value_id == id {
                    match value {
                        Value::Map(entry) => {
                            entry.make_unique();
                        }
                        Value::List(entry) => {
                            entry.make_unique();
                        }
                        _ => {}
                    }
                    return Some(value.clone());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callstack() {
        use Value::*;

        let mut stack = CallStack::with_capacity(32);

        assert_eq!(stack.frame(), 0);
        assert_eq!(stack.get("foo"), None);

        stack.push(Id::from_str("foo"), Number(42.0));
        stack.push(Id::from_str("bar"), Number(99.0));
        stack.commit();

        assert_eq!(stack.frame(), 1);
        assert_eq!(stack.get("foo"), Some(&Number(42.0)));
        assert_eq!(stack.get("bar"), Some(&Number(99.0)));

        stack.push(Id::from_str("baz"), Number(-1.0));
        // We should be able to access the previous frame values while preparing the next
        assert_eq!(stack.get("foo"), Some(&Number(42.0)));
        stack.commit();

        assert_eq!(stack.frame(), 2);
        assert_eq!(stack.get("foo"), None);
        assert_eq!(stack.get("bar"), None);
        assert_eq!(stack.get("baz"), Some(&Number(-1.0)));

        stack.extend(Id::from_str("qux"), Number(100.0));
        assert_eq!(stack.get("qux"), Some(&Number(100.0)));

        stack.pop_frame();

        assert_eq!(stack.frame(), 1);
        assert_eq!(stack.get("foo"), Some(&Number(42.0)));
        *stack.get_mut("bar").unwrap() = Number(7.0);
        assert_eq!(stack.get("bar"), Some(&Number(7.0)));
        assert_eq!(stack.get("baz"), None);
        assert_eq!(stack.get("qux"), None);

        stack.push(Id::from_str("baz"), Number(-1.0));

        stack.cancel();
        assert_eq!(stack.frame(), 1);
        assert_eq!(stack.get("baz"), None);

        stack.pop_frame();

        assert_eq!(stack.frame(), 0);
        assert_eq!(stack.get("foo"), None);
        assert_eq!(stack.get("bar"), None);
        assert_eq!(stack.get("baz"), None);
    }
}
