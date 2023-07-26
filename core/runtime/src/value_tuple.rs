use {
    crate::{prelude::*, Result},
    std::ops::{Deref, Range},
};

/// The Tuple type used by the Koto runtime
#[derive(Clone, Debug)]
pub struct ValueTuple {
    data: Ptr<[Value]>,
    bounds: Range<usize>,
}

impl ValueTuple {
    /// Returns a new tuple with shared data and with restricted bounds
    ///
    /// The provided bounds should have indices relative to the current tuple's bounds
    /// (i.e. instead of relative to the underlying shared tuple data), so it follows that the
    /// result will always be a subset of the input tuple.
    pub fn make_sub_tuple(&self, mut new_bounds: Range<usize>) -> Option<Self> {
        new_bounds.start += self.bounds.start;
        new_bounds.end += self.bounds.start;

        if new_bounds.end <= self.bounds.end && self.data.get(new_bounds.clone()).is_some() {
            Some(Self {
                data: self.data.clone(),
                bounds: new_bounds,
            })
        } else {
            None
        }
    }

    /// Returns true if the tuple contains only immutable values
    pub fn is_hashable(&self) -> bool {
        self.iter().all(Value::is_hashable)
    }

    /// Removes and returns the first value in the tuple
    ///
    /// The internal bounds of the tuple are adjusted to 'remove' the first element;
    /// no change is made to the underlying tuple data.
    pub fn pop_front(&mut self) -> Option<Value> {
        if let Some(value) = self.first().cloned() {
            self.bounds.start += 1;
            Some(value)
        } else {
            None
        }
    }

    /// Removes and returns the last value in the tuple
    ///
    /// The internal bounds of the tuple are adjusted to 'remove' the first element;
    /// no change is made to the underlying tuple data.
    pub fn pop_back(&mut self) -> Option<Value> {
        if let Some(value) = self.last().cloned() {
            self.bounds.end -= 1;
            Some(value)
        } else {
            None
        }
    }
}

impl Deref for ValueTuple {
    type Target = [Value];

    fn deref(&self) -> &[Value] {
        // Safety: bounds have already been checked in the From impls and make_sub_tuple
        unsafe { self.data.get_unchecked(self.bounds.clone()) }
    }
}

impl Default for ValueTuple {
    fn default() -> Self {
        Self {
            data: Vec::default().into(),
            bounds: Range::default(),
        }
    }
}

impl KotoDisplay for ValueTuple {
    fn display(&self, s: &mut String, vm: &mut Vm, _options: KotoDisplayOptions) -> Result<()> {
        s.push('(');
        for (i, value) in self.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            value.display(
                s,
                vm,
                KotoDisplayOptions {
                    contained_value: true,
                },
            )?;
        }
        s.push(')');

        Ok(())
    }
}

impl From<&[Value]> for ValueTuple {
    fn from(data: &[Value]) -> Self {
        let bounds = 0..data.len();
        Self {
            data: data.into(),
            bounds,
        }
    }
}

impl From<Vec<Value>> for ValueTuple {
    fn from(data: Vec<Value>) -> Self {
        let bounds = 0..data.len();
        Self {
            data: data.into(),
            bounds,
        }
    }
}
