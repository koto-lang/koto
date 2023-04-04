use {
    crate::prelude::*,
    std::{cmp::Ordering, fmt, hash::Hash, ops::Range},
};

/// The integer range type used by the Koto runtime
///
/// See [Value::Range]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct IntRange {
    /// The optional start of the range
    pub start: Option<isize>,
    /// The optional end of the range, along with a flag for inclusive ranges
    pub end: Option<(isize, bool)>,
}

impl IntRange {
    /// Initializes a range with the given bounds
    pub fn with_bounds(start: isize, end: isize, inclusive: bool) -> Self {
        Self {
            start: Some(start),
            end: Some((end, inclusive)),
        }
    }

    /// Returns a sorted translation of the range with missing boundaries replaced by min/max values
    ///
    /// No clamping of the range boundaries is performed (as in [IntRange::indices]),
    /// so negative indices will be preserved.
    pub fn as_sorted_range(&self) -> Range<isize> {
        use std::isize::{MAX, MIN};

        match (self.start, self.end) {
            (Some(start), Some((end, inclusive))) => match (start <= end, inclusive) {
                (true, true) => start..end + 1,
                (true, false) => start..end,
                (false, true) => end..start + 1,
                (false, false) => end + 1..start + 1,
            },
            (Some(start), None) => start..MAX,
            (None, Some((end, inclusive))) => {
                if inclusive {
                    MIN..end + 1
                } else {
                    MIN..end
                }
            }
            (None, None) => MIN..MAX,
        }
    }

    /// Returns true if the provided number is within the range
    pub fn contains(&self, n: ValueNumber) -> bool {
        let n: isize = if n < 0.0 { n.floor() } else { n.ceil() }.into();
        match (self.start, self.end) {
            (None, None) => true,
            (None, Some((end, true))) => n <= end,
            (None, Some((end, false))) => n < end,
            (Some(start), None) => n >= start,
            (Some(start), Some((end, true))) => {
                if start <= end {
                    n >= start && n <= end
                } else {
                    n <= start && n >= end
                }
            }
            (Some(start), Some((end, false))) => {
                if start <= end {
                    n >= start && n < end
                } else {
                    n <= start && n > end
                }
            }
        }
    }

    /// Returns the range translated into non-negative indices, suitable for container access
    ///
    /// The start index will be clamped to the range `0..=max_index`.
    /// The end index will be clamped to the range `start..=max_index`
    ///
    /// If the start value is `None` then the resulting start index will be `0`.
    /// If the end value is `None` then the resulting end index will be `max_index`.
    pub fn indices(&self, max_index: usize) -> Range<usize> {
        let start = self
            .start
            .map_or(0, |start| (start.max(0) as usize).min(max_index));

        let end = self.end.map_or(max_index, |(end, inclusive)| {
            let end = if inclusive { end + 1 } else { end } as usize;
            end.clamp(0, max_index)
        });

        start..end
    }

    /// Returns the intersection of two ranges
    pub fn intersection(&self, other: IntRange) -> Option<Self> {
        let this = self.as_sorted_range();
        // let mut result = Self::with_bounds(start, end, inclusive);
        let other = other.as_sorted_range();

        if !(this.contains(&other.start) || this.contains(&other.end)) {
            return None;
        }

        Some(Self::with_bounds(
            this.start.max(other.start),
            this.end.min(other.end),
            false,
        ))
    }

    /// Returns true if the range's start is less than or equal to its end
    pub fn is_ascending(&self) -> bool {
        match (self.start, self.end) {
            (Some(start), Some((end, _))) => start <= end,
            _ => true,
        }
    }

    /// Returns the size of the range if both start and end boundaries are specified
    ///
    /// Descending ranges have a non-negative size, i.e. the size is equal to `start - end`.
    pub fn size(&self) -> Option<usize> {
        if self.start.is_some() && self.end.is_some() {
            Some(self.as_sorted_range().len())
        } else {
            None
        }
    }

    /// Returns true if the range has defined start and end boundaries
    pub fn is_bounded(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }

    /// Removes and returns the first element in the range.
    ///
    /// This is used in the VM to iterate over temporary ranges.
    ///
    /// Returns an error if the range is not bounded.
    pub fn pop_front(&mut self) -> Result<Option<isize>, RuntimeError> {
        let result = match (self.start, self.end) {
            (Some(start), Some((end, inclusive))) => match (start.cmp(&end), inclusive) {
                (Ordering::Less, _) => {
                    self.start = Some(start + 1);
                    Some(start)
                }
                (Ordering::Greater, _) => {
                    self.start = Some(start - 1);
                    Some(start)
                }
                // An inclusive range, with start == end
                (Ordering::Equal, true) => {
                    self.end = Some((end, false));
                    Some(start)
                }
                // The range is exhausted
                (Ordering::Equal, false) => None,
            },
            _ => return runtime_error!("IntRange::pop_front can only be used with bounded ranges"),
        };

        Ok(result)
    }
}

impl fmt::Display for IntRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(start) = self.start {
            write!(f, "{start}")?;
        }

        f.write_str("..")?;

        if let Some((end, inclusive)) = self.end {
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
    fn as_sorted_range() {
        use std::isize::{MAX, MIN};

        assert_eq!(
            10..20,
            IntRange::with_bounds(10, 20, false).as_sorted_range()
        );
        assert_eq!(
            10..21,
            IntRange::with_bounds(10, 20, true).as_sorted_range()
        );
        assert_eq!(
            11..21,
            IntRange::with_bounds(20, 10, false).as_sorted_range()
        );
        assert_eq!(
            10..21,
            IntRange::with_bounds(20, 10, true).as_sorted_range()
        );

        assert_eq!(
            10..MAX,
            IntRange {
                start: Some(10),
                end: None
            }
            .as_sorted_range(),
        );
        assert_eq!(
            MIN..10,
            IntRange {
                start: None,
                end: Some((10, false))
            }
            .as_sorted_range(),
        );
    }

    #[test]
    fn intersection() {
        assert_eq!(
            Some(IntRange::with_bounds(15, 20, false)),
            IntRange::with_bounds(10, 20, false).intersection(IntRange::with_bounds(15, 25, false))
        );
        assert_eq!(
            Some(IntRange::with_bounds(200, 201, false)),
            IntRange::with_bounds(100, 200, true)
                .intersection(IntRange::with_bounds(300, 200, true))
        );
        assert_eq!(
            None,
            IntRange::with_bounds(100, 200, false)
                .intersection(IntRange::with_bounds(0, 50, false))
        );
    }

    #[test]
    fn is_ascending() {
        assert!(IntRange::with_bounds(10, 20, false).is_ascending());
        assert!(!IntRange::with_bounds(30, 20, false).is_ascending());
        assert!(IntRange {
            start: None,
            end: Some((1, true))
        }
        .is_ascending());
        assert!(IntRange {
            start: Some(20),
            end: None
        }
        .is_ascending());
    }
}
