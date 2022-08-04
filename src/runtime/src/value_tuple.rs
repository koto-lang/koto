use {
    crate::Value,
    std::{
        fmt,
        ops::{Deref, Range},
        rc::Rc,
    },
};

#[derive(Clone, Debug)]
pub struct ValueTuple {
    data: Rc<[Value]>,
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

impl fmt::Display for ValueTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, value) in self.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{value:#}")?;
        }
        write!(f, ")")
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
