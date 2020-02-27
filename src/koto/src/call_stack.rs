use crate::{Id, Value};

pub struct CallStack<'a> {
    values: Vec<(Id, Value<'a>)>,
    frame_size: Vec<usize>,
    frame_value_count: usize,
}

impl<'a> CallStack<'a> {
    pub fn new() -> Self {
        let initial_capacity = 32;
        Self {
            values: Vec::with_capacity(initial_capacity),
            frame_size: Vec::with_capacity(initial_capacity),
            frame_value_count: 0,
        }
    }

    pub fn frame(&self) -> usize {
        self.frame_size.len()
    }

    pub fn push(&mut self, id: Id, value: Value<'a>) {
        self.values.push((id, value));
        self.frame_value_count += 1;
    }

    pub fn extend(&mut self, id: Id, value: Value<'a>) {
        assert_eq!(
            self.frame_value_count, 0,
            "Extend called before commit or cancel"
        );
        self.values.push((id, value));
        *self
            .frame_size
            .last_mut()
            .expect("Extend called before commiting a frame") += 1;
    }

    pub fn commit(&mut self) {
        self.frame_size.push(self.frame_value_count);
        self.frame_value_count = 0;
    }

    pub fn cancel(&mut self) {
        for _ in 0..self.frame_value_count {
            self.values.pop();
        }
        self.frame_value_count = 0;
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
                let values_start = self.values.len() - size;
                Some(&self.values[values_start..])
            }
            None => None,
        }
    }

    fn frame_values_mut(&mut self) -> Option<&mut [(Id, Value<'a>)]> {
        match self.frame_size.last() {
            Some(size) => {
                let values_start = self.values.len() - size;
                Some(&mut self.values[values_start..])
            }
            None => None,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Value<'a>> {
        match self.frame_values() {
            Some(values) => values.iter().find_map(|(value_id, value)| {
                if value_id.as_ref() == id {
                    Some(value)
                } else {
                    None
                }
            }),
            None => None,
        }
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Value<'a>> {
        match self.frame_values_mut() {
            Some(values) => values.iter_mut().find_map(|(value_id, value)| {
                if value_id.as_ref() == id {
                    Some(value)
                } else {
                    None
                }
            }),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callstack() {
        use std::rc::Rc;
        use Value::*;

        let mut stack = CallStack::new();

        assert_eq!(stack.frame(), 0);
        assert_eq!(stack.get("foo"), None);

        stack.push(Rc::new("foo".to_string()), Number(42.0));
        stack.push(Rc::new("bar".to_string()), Number(99.0));
        stack.commit();

        assert_eq!(stack.frame(), 1);
        assert_eq!(stack.get("foo"), Some(&Number(42.0)));
        assert_eq!(stack.get("bar"), Some(&Number(99.0)));

        stack.push(Rc::new("baz".to_string()), Number(-1.0));
        stack.commit();

        assert_eq!(stack.frame(), 2);
        assert_eq!(stack.get("foo"), None);
        assert_eq!(stack.get("bar"), None);
        assert_eq!(stack.get("baz"), Some(&Number(-1.0)));

        stack.extend(Rc::new("qux".to_string()), Number(100.0));
        assert_eq!(stack.get("qux"), Some(&Number(100.0)));

        stack.pop_frame();

        assert_eq!(stack.frame(), 1);
        assert_eq!(stack.get("foo"), Some(&Number(42.0)));
        *stack.get_mut("bar").unwrap() = Number(7.0);
        assert_eq!(stack.get("bar"), Some(&Number(7.0)));
        assert_eq!(stack.get("baz"), None);
        assert_eq!(stack.get("qux"), None);

        stack.push(Rc::new("baz".to_string()), Number(-1.0));

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
