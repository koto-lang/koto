use crate::{prelude::*, Error};
use std::{cmp::Ordering, fmt, hash::Hash, ops::Range};

/// The integer range type used by the Koto runtime
///
/// See [Value::Range]
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
    /// Initializes a From range
    pub fn from(start: i64) -> Self {
        Self(Inner::From { start })
    }

    /// Initializes a To range
    pub fn to(end: i64, inclusive: bool) -> Self {
        Self(Inner::To { end, inclusive })
    }

    /// Initializes a range with the given bounds
    pub fn bounded(start: i64, end: i64, inclusive: bool) -> Self {
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

    /// Initializes an unbounded range
    pub fn unbounded() -> Self {
        Self(Inner::Unbounded)
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

    /// Returns a sorted translation of the range with missing boundaries replaced by min/max values
    ///
    /// No clamping of the range boundaries is performed (as in [KRange::indices]),
    /// so negative indices will be preserved.
    pub fn as_sorted_range(&self) -> Range<i64> {
        use std::i64::{MAX, MIN};
        use Inner::*;

        let sort_bounded = |start, end, inclusive| {
            if start < end {
                (start, if inclusive { end + 1 } else { end })
            } else {
                (if inclusive { end } else { end + 1 }, start + 1)
            }
        };

        let (start, end) = {
            match &self.0 {
                From { start } => (*start, MAX),
                To { end, inclusive } => (MIN, if *inclusive { *end + 1 } else { *end }),
                Bounded {
                    start,
                    end,
                    inclusive,
                } => sort_bounded(*start as i64, *end as i64, *inclusive),
                BoundedLarge(r) => sort_bounded(r.start, r.end, r.inclusive),
                Unbounded => (MIN, MAX),
            }
        };

        start..end
    }

    /// Returns true if the provided number is within the range
    pub fn contains(&self, n: KNumber) -> bool {
        let n: i64 = if n < 0.0 { n.floor() } else { n.ceil() }.into();
        self.as_sorted_range().contains(&n)
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
        let range = self.as_sorted_range();
        let start = range.start.clamp(0, max_index);
        let end = range.end.clamp(start, max_index);
        (start as usize)..(end as usize)
    }

    /// Returns the intersection of two ranges
    pub fn intersection(&self, other: &KRange) -> Option<Self> {
        let this = self.as_sorted_range();
        // let mut result = Self::with_bounds(start, end, inclusive);
        let other = other.as_sorted_range();

        if !(this.contains(&other.start) || this.contains(&other.end)) {
            return None;
        }

        Some(Self::bounded(
            this.start.max(other.start),
            this.end.min(other.end),
            false,
        ))
    }

    /// Returns true if the range's start is less than or equal to its end
    pub fn is_ascending(&self) -> bool {
        use Inner::*;
        match &self.0 {
            To { end, .. } => *end > 0,
            Bounded { start, end, .. } => *start <= *end,
            BoundedLarge(r) => r.start <= r.end,
            _ => true,
        }
    }

    /// Returns the size of the range if both start and end boundaries are specified
    ///
    /// Descending ranges have a non-negative size, i.e. the size is equal to `start - end`.
    pub fn size(&self) -> Option<usize> {
        if self.is_bounded() {
            let range = self.as_sorted_range();
            Some((range.end - range.start) as usize)
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
                Greater => {
                    let result = *start as i64;
                    *start -= 1;
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
            },
            BoundedLarge(r) => {
                let r = Ptr::make_mut(r);
                match r.start.cmp(&r.end) {
                    Less => {
                        let result = r.start;
                        r.start += 1;
                        Some(result)
                    }
                    Greater => {
                        let result = r.start;
                        r.start -= 1;
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
                }
            }
            _ => return runtime_error!("KRange::pop_front can only be used with bounded ranges"),
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
                    let result = *end as i64;
                    *end -= 1;
                    Some(result)
                }
                Greater => {
                    let result = *start as i64;
                    *start -= 1;
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
            },
            BoundedLarge(r) => {
                let r = Ptr::make_mut(r);
                match r.start.cmp(&r.end) {
                    Less => {
                        let result = r.end;
                        r.end += 1;
                        Some(result)
                    }
                    Greater => {
                        let result = r.start;
                        r.start -= 1;
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
                }
            }
            _ => return runtime_error!("KRange::pop_back can only be used with bounded ranges"),
        };

        Ok(result)
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
mod tests {
    use super::*;

    #[test]
    fn size_of() {
        assert_eq!(std::mem::size_of::<KRange>(), 16);
    }

    #[test]
    fn as_sorted_range() {
        use std::i64::{MAX, MIN};

        assert_eq!(10..20, KRange::bounded(10, 20, false).as_sorted_range());
        assert_eq!(10..21, KRange::bounded(10, 20, true).as_sorted_range());
        assert_eq!(11..21, KRange::bounded(20, 10, false).as_sorted_range());
        assert_eq!(10..21, KRange::bounded(20, 10, true).as_sorted_range());

        assert_eq!(10..MAX, KRange::from(10).as_sorted_range(),);
        assert_eq!(MIN..10, KRange::to(10, false).as_sorted_range(),);
    }

    #[test]
    fn intersection() {
        assert_eq!(
            Some(KRange::bounded(15, 20, false)),
            KRange::bounded(10, 20, false).intersection(&KRange::bounded(15, 25, false))
        );
        assert_eq!(
            Some(KRange::bounded(200, 201, false)),
            KRange::bounded(100, 200, true).intersection(&KRange::bounded(300, 200, true))
        );
        assert_eq!(
            None,
            KRange::bounded(100, 200, false).intersection(&KRange::bounded(0, 50, false))
        );
    }

    #[test]
    fn is_ascending() {
        assert!(KRange::bounded(10, 20, false).is_ascending());
        assert!(!KRange::bounded(30, 20, false).is_ascending());
        assert!(KRange::to(1, true).is_ascending());
        assert!(KRange::from(20).is_ascending());
    }

    #[test]
    fn bounded_large() {
        let start_big = 2_i64.pow(42);
        let end_big = 2_i64.pow(43);
        assert!(KRange::bounded(start_big, end_big, false).is_ascending());
        assert_eq!(
            KRange::bounded(start_big, end_big, false).size().unwrap(),
            (end_big - start_big) as usize
        );
    }
}
