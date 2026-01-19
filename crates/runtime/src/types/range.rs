use crate::{Error, Ptr, prelude::*};
use std::{
    cmp::Ordering,
    fmt,
    hash::Hash,
    ops::{Range, RangeBounds},
};

/// The integer range type used by the Koto runtime
///
/// See [`KValue::Range`]
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct KRange(Inner);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum Inner {
    Unbounded,
    From {
        start: i64,
    },
    To {
        end: i64,
        inclusive: bool,
    },
    Bounded {
        start: i32,
        end: i32,
        inclusive: bool,
    },
    // Placing ranges with i64 bounds to the heap allows the size of KRange to be 16 bytes
    BoundedLarge(Ptr<Bounded64>),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Bounded64 {
    start: i64,
    end: i64,
    inclusive: bool,
}

impl From<Bounded64> for Inner {
    fn from(range: Bounded64) -> Self {
        Self::BoundedLarge(range.into())
    }
}

impl KRange {
    /// Initializes a range with the given start and end bounds.
    ///
    /// The end bound has an additional flag that specifies whether or not the end of the range is
    /// inclusive or not.
    ///
    /// `KRange` has an implementation of `From` for implementations of `RangeBounds` which might be
    /// simpler to use, e.g. `KRange::from(10..=20)`
    pub fn new(start: Option<i64>, end: Option<(i64, bool)>) -> Self {
        match (start, end) {
            (Some(start), Some((end, inclusive))) => {
                match (i32::try_from(start), i32::try_from(end)) {
                    (Ok(start), Ok(end)) => Self(Inner::Bounded {
                        start,
                        end,
                        inclusive,
                    }),
                    _ => Self(
                        Bounded64 {
                            start,
                            end,
                            inclusive,
                        }
                        .into(),
                    ),
                }
            }
            (Some(start), None) => Self(Inner::From { start }),
            (None, Some((end, inclusive))) => Self(Inner::To { end, inclusive }),
            (None, None) => Self(Inner::Unbounded),
        }
    }

    /// Returns the start of the range
    pub fn start(&self) -> Option<i64> {
        use Inner::*;
        match &self.0 {
            From { start } => Some(*start),
            Bounded { start, .. } => Some(*start as i64),
            BoundedLarge(r) => Some(r.start),
            _ => None,
        }
    }

    /// Returns the end of the range
    ///
    /// The return value includes flag stating whether or not the range end is inclusive or not.
    pub fn end(&self) -> Option<(i64, bool)> {
        use Inner::*;
        match &self.0 {
            To { end, inclusive } => Some((*end, *inclusive)),
            Bounded { end, inclusive, .. } => Some((*end as i64, *inclusive)),
            BoundedLarge(r) => Some((r.end, r.inclusive)),
            _ => None,
        }
    }

    /// Returns the range with missing boundaries replaced by min/max values.
    ///
    /// Inclusive ranges are converted into non-inclusive ranges.
    ///
    /// No clamping of the range boundaries is performed (as in [KRange::indices]),
    /// so negative indices will be preserved.
    pub fn as_bounded_range(&self) -> Range<i64> {
        let (start, end, inclusive) = {
            match &self.0 {
                Inner::From { start } => (*start, i64::MAX, false),
                Inner::To { end, inclusive } => (i64::MIN, *end, *inclusive),
                Inner::Bounded {
                    start,
                    end,
                    inclusive,
                } => (*start as i64, *end as i64, *inclusive),
                Inner::BoundedLarge(r) => (r.start, r.end, r.inclusive),
                Inner::Unbounded => (i64::MIN, i64::MAX, false),
            }
        };

        let end = if inclusive { end + 1 } else { end };
        start..end.max(start)
    }

    /// Returns true if the provided number is within the range
    pub fn contains(&self, n: KNumber) -> bool {
        let n: i64 = if n < 0.0 { n.floor() } else { n.ceil() }.into();
        self.as_bounded_range().contains(&n)
    }

    /// Returns the range translated into non-negative indices, suitable for container access
    ///
    /// The start index will be clamped to the range `0..=max_index`.
    /// The end index will be clamped to the range `start..=max_index`
    ///
    /// If the start value is `None` then the resulting start index will be `0`.
    /// If the end value is `None` then the resulting end index will be `max_index`.
    pub fn indices(&self, max_index: usize) -> Range<usize> {
        let max_index = max_index as i64;
        let range = self.as_bounded_range();
        let start = range.start.clamp(0, max_index);
        let end = range.end.clamp(start, max_index);
        (start as usize)..(end as usize)
    }

    /// Returns the intersection of two ranges
    pub fn intersection(&self, other: &KRange) -> Option<Self> {
        let this = self.as_bounded_range();
        let other = other.as_bounded_range();

        if !(this.contains(&other.start) || this.contains(&other.end)) {
            return None;
        }

        Some(Self::from(
            this.start.max(other.start)..this.end.min(other.end),
        ))
    }

    /// Returns the size of the range if both start and end boundaries are specified
    ///
    /// Descending ranges are considered to be empty, with a size of zero.
    pub fn size(&self) -> Option<usize> {
        if self.is_bounded() {
            let range = self.as_bounded_range();
            Some(((range.end).max(range.start) - range.start) as usize)
        } else {
            None
        }
    }

    /// Returns true if the range has defined start and end boundaries
    pub fn is_bounded(&self) -> bool {
        use Inner::*;
        matches!(self.0, Bounded { .. } | BoundedLarge { .. })
    }

    /// Removes and returns the first element in the range.
    ///
    /// This is used by RangeIterator and in the VM to iterate over temporary ranges.
    ///
    /// Returns an error if the range is not bounded.
    pub fn pop_front(&mut self) -> Result<Option<i64>, Error> {
        use Inner::*;
        use Ordering::*;

        let result = match &mut self.0 {
            Bounded {
                start,
                end,
                inclusive,
            } => match start.cmp(&end) {
                Less => {
                    let result = *start as i64;
                    *start += 1;
                    Some(result)
                }
                Equal => {
                    if *inclusive {
                        let result = *start as i64;
                        *inclusive = false; // Allow iteration to stop
                        Some(result)
                    } else {
                        None
                    }
                }
                Greater => None,
            },
            BoundedLarge(r) => {
                let r = Ptr::make_mut(r);
                match r.start.cmp(&r.end) {
                    Less => {
                        let result = r.start;
                        r.start += 1;
                        Some(result)
                    }
                    Equal => {
                        if r.inclusive {
                            let result = r.start;
                            r.inclusive = false; // Allow iteration to stop
                            Some(result)
                        } else {
                            None
                        }
                    }
                    Greater => None,
                }
            }
            _ => return runtime_error!("expected a bounded range"),
        };

        Ok(result)
    }

    /// Removes and returns the first element in the range.
    ///
    /// This is used by RangeIterator and in the VM to iterate over temporary ranges.
    ///
    /// Returns an error if the range is not bounded.
    pub fn pop_back(&mut self) -> Result<Option<i64>, Error> {
        use Inner::*;
        use Ordering::*;

        let result = match &mut self.0 {
            Bounded {
                start,
                end,
                inclusive,
            } => match start.cmp(&end) {
                Less => {
                    let result = if *inclusive { *end } else { *end - 1 } as i64;
                    *end -= 1;
                    Some(result)
                }
                Equal => {
                    if *inclusive {
                        let result = *start as i64;
                        *inclusive = false; // Allow iteration to stop
                        Some(result)
                    } else {
                        None
                    }
                }
                Greater => None,
            },
            BoundedLarge(r) => {
                let r = Ptr::make_mut(r);
                match r.start.cmp(&r.end) {
                    Less => {
                        let result = if r.inclusive { r.end } else { r.end - 1 };
                        r.end -= 1;
                        Some(result)
                    }
                    Equal => {
                        if r.inclusive {
                            let result = r.start;
                            r.inclusive = false; // Allow iteration to stop
                            Some(result)
                        } else {
                            None
                        }
                    }
                    Greater => None,
                }
            }
            _ => return runtime_error!("expected a bounded range"),
        };

        Ok(result)
    }
}

impl<R> From<R> for KRange
where
    R: RangeBounds<i64>,
{
    fn from(range: R) -> Self {
        use std::ops::Bound::*;

        let (start, end) = match (range.start_bound(), range.end_bound()) {
            (Included(&start), Included(&end)) => (Some(start), Some((end, true))),
            (Included(&start), Excluded(&end)) => (Some(start), Some((end, false))),
            (Included(&start), Unbounded) => (Some(start), None),
            (Unbounded, Included(&end)) => (None, Some((end, true))),
            (Unbounded, Excluded(&end)) => (None, Some((end, false))),
            (Unbounded, Unbounded) => (None, None),
            _ => unimplemented!(),
        };

        Self::new(start, end)
    }
}

impl fmt::Display for KRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(start) = self.start() {
            write!(f, "{start}")?;
        }

        f.write_str("..")?;

        if let Some((end, inclusive)) = self.end() {
            if inclusive {
                f.write_str("=")?;
            }
            write!(f, "{end}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::reversed_empty_ranges)]
mod tests {
    use super::*;

    #[test]
    fn size_of() {
        assert_eq!(std::mem::size_of::<KRange>(), 16);
    }

    #[test]
    fn as_bounded_range() {
        assert_eq!(10..20, KRange::from(10..20).as_bounded_range());
        assert_eq!(10..21, KRange::from(10..=20).as_bounded_range());
        assert_eq!(20..20, KRange::from(20..10).as_bounded_range());
        assert_eq!(20..20, KRange::from(20..=10).as_bounded_range());

        assert_eq!(10..i64::MAX, KRange::from(10..).as_bounded_range(),);
        assert_eq!(i64::MIN..10, KRange::from(..10).as_bounded_range(),);
    }

    #[test]
    fn intersection() {
        assert_eq!(
            Some(KRange::from(15..20)),
            KRange::from(10..20).intersection(&KRange::from(15..25))
        );
        assert_eq!(
            None,
            KRange::from(100..=200).intersection(&KRange::from(300..=200))
        );
        assert_eq!(
            None,
            KRange::from(100..200).intersection(&KRange::from(0..50))
        );
    }

    #[test]
    fn bounded_large() {
        let start_big = 2_i64.pow(42);
        let end_big = 2_i64.pow(43);
        assert_eq!(
            KRange::from(start_big..end_big).size().unwrap(),
            (end_big - start_big) as usize
        );
    }
}
