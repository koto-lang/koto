use crate::{prelude::*, Ptr, Result};
use std::ops::{Deref, Range};

/// The Tuple type used by the Koto runtime
#[derive(Clone)]
pub struct KTuple(Inner);

// Either the full tuple, or a slice
//
// By heap-allocating slice bounds we can keep KTuple's size down to 16 bytes; otherwise it
// would have a size of 32 bytes.
#[derive(Clone)]
enum Inner {
    Full(Ptr<[KValue]>),
    Slice(Ptr<TupleSlice>),
}

#[derive(Clone)]
struct TupleSlice {
    data: Ptr<[KValue]>,
    bounds: Range<usize>,
}

impl KTuple {
    /// Returns a new tuple with shared data and with restricted bounds
    ///
    /// The provided bounds should have indices relative to the current tuple's bounds
    /// (i.e. instead of relative to the underlying shared tuple data), so it follows that the
    /// result will always be a subset of the input tuple.
    pub fn make_sub_tuple(&self, mut new_bounds: Range<usize>) -> Option<Self> {
        let slice = match &self.0 {
            Inner::Full(data) => TupleSlice::from(data.clone()),
            Inner::Slice(slice) => slice.deref().clone(),
        };

        new_bounds.start += slice.bounds.start;
        new_bounds.end += slice.bounds.start;

        if new_bounds.end <= slice.bounds.end && slice.get(new_bounds.clone()).is_some() {
            let result = TupleSlice {
                data: slice.data.clone(),
                bounds: new_bounds,
            };
            Some(result.into())
        } else {
            None
        }
    }

    /// Returns true if the tuple contains only immutable values
    pub fn is_hashable(&self) -> bool {
        self.iter().all(KValue::is_hashable)
    }

    /// Removes and returns the first value in the tuple
    ///
    /// The internal bounds of the tuple are adjusted to 'remove' the first element;
    /// no change is made to the underlying tuple data.
    pub fn pop_front(&mut self) -> Option<KValue> {
        match &mut self.0 {
            Inner::Full(data) => {
                if let Some(value) = data.first().cloned() {
                    *self = Self::from(TupleSlice {
                        data: data.clone(),
                        bounds: 1..data.len(),
                    });
                    Some(value)
                } else {
                    None
                }
            }
            Inner::Slice(slice) => {
                if let Some(value) = slice.first().cloned() {
                    Ptr::make_mut(slice).bounds.start += 1;
                    Some(value)
                } else {
                    None
                }
            }
        }
    }

    /// Removes and returns the last value in the tuple
    ///
    /// The internal bounds of the tuple are adjusted to 'remove' the first element;
    /// no change is made to the underlying tuple data.
    pub fn pop_back(&mut self) -> Option<KValue> {
        match &mut self.0 {
            Inner::Full(data) => {
                if let Some(value) = data.last().cloned() {
                    *self = Self::from(TupleSlice {
                        data: data.clone(),
                        bounds: 0..data.len() - 1,
                    });
                    Some(value)
                } else {
                    None
                }
            }
            Inner::Slice(slice) => {
                if let Some(value) = slice.last().cloned() {
                    Ptr::make_mut(slice).bounds.end -= 1;
                    Some(value)
                } else {
                    None
                }
            }
        }
    }

    /// Renders the tuple into the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        let id = Ptr::address(match &self.0 {
            Inner::Full(data) => data,
            Inner::Slice(slice) => &slice.data,
        });
        ctx.push_container(id);
        ctx.append('(');

        for (i, value) in self.iter().enumerate() {
            if i > 0 {
                ctx.append(", ");
            }
            value.display(ctx)?;
        }

        ctx.append(')');
        ctx.pop_container();

        Ok(())
    }
}

impl Deref for KTuple {
    type Target = [KValue];

    fn deref(&self) -> &[KValue] {
        match &self.0 {
            Inner::Full(data) => data,
            Inner::Slice(slice) => slice.deref(),
        }
    }
}

impl Default for KTuple {
    fn default() -> Self {
        Vec::new().into()
    }
}

impl From<Vec<KValue>> for KTuple {
    fn from(data: Vec<KValue>) -> Self {
        Self(Inner::Full(data.into()))
    }
}

impl From<&[KValue]> for KTuple {
    fn from(data: &[KValue]) -> Self {
        Self(Inner::Full(data.into()))
    }
}

impl<const N: usize> From<&[KValue; N]> for KTuple {
    fn from(data: &[KValue; N]) -> Self {
        Self::from(data.as_slice())
    }
}

impl Deref for TupleSlice {
    type Target = [KValue];

    fn deref(&self) -> &[KValue] {
        // Safety: bounds have already been checked in the From impls and make_sub_tuple
        unsafe { self.data.get_unchecked(self.bounds.clone()) }
    }
}

impl From<Ptr<[KValue]>> for TupleSlice {
    fn from(data: Ptr<[KValue]>) -> Self {
        let bounds = 0..data.len();
        Self { data, bounds }
    }
}

impl From<TupleSlice> for KTuple {
    fn from(slice: TupleSlice) -> Self {
        Self(Inner::Slice(slice.into()))
    }
}
