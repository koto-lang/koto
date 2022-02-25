use {
    std::{
        fmt,
        hash::{Hash, Hasher},
        ops::{Deref, Range},
        rc::Rc,
    },
    unicode_segmentation::UnicodeSegmentation,
};

#[derive(Clone)]
pub struct ValueString {
    string: Rc<str>,
    bounds: Range<usize>,
}

impl ValueString {
    fn new(string: Rc<str>) -> Self {
        let bounds = 0..string.len();
        Self { string, bounds }
    }

    /// Returns the empty string
    pub fn empty() -> Self {
        Self::new(EMPTY_STRING.with(|s| s.clone()))
    }

    pub fn new_with_bounds(string: Rc<str>, bounds: Range<usize>) -> Option<Self> {
        if string.get(bounds.clone()).is_some() {
            Some(Self { string, bounds })
        } else {
            None
        }
    }

    pub fn with_bounds(&self, mut new_bounds: Range<usize>) -> Option<Self> {
        new_bounds.end += self.bounds.start;
        new_bounds.start += self.bounds.start;

        if new_bounds.end <= self.bounds.end && self.string.get(new_bounds.clone()).is_some() {
            Some(Self {
                string: self.string.clone(),
                bounds: new_bounds,
            })
        } else {
            None
        }
    }

    pub fn with_grapheme_indices(&self, start: usize, end: Option<usize>) -> Option<Self> {
        let end_unwrapped = end.unwrap_or(self.len());
        debug_assert!(start <= end_unwrapped);

        let mut result_start = if start == 0 { Some(0) } else { None };
        let mut result_end = None;

        for (i, (grapheme_start, grapheme)) in self.grapheme_indices(true).enumerate() {
            if result_start.is_none() && i == start - 1 {
                // By checking against start - 1 (rather than waiting until the next iteration),
                // we can allow for indexing from 'one past the end' to get to an empty string,
                // which can be useful when consuming characters from a string.
                // e.g.
                //   x = get_string()
                //   do_something_with_first_char x[0]
                //   do_something_with_remaining_string x[1..]
                result_start = Some(grapheme_start + grapheme.len());

                if end.is_none() {
                    break;
                }
            }

            if i == end_unwrapped - 1 {
                if start == end_unwrapped {
                    // The start index has been validated, so just return the empty string
                    return Some(Self::empty());
                }

                // Checking against end - 1 in the same way as for result_start,
                // allowing for indexing one-past-the-end.
                // e.g. assert_eq 'xyz'[1..3], 'yz'
                result_end = Some(grapheme_start + grapheme.len());
                break;
            }
        }

        let result_bounds = match (result_start, result_end) {
            (Some(result_start), Some(result_end)) => result_start..result_end,
            (Some(result_start), None) if end.is_none() => result_start..self.len(),
            _ => return None,
        };

        self.with_bounds(result_bounds)
    }

    pub fn grapheme_count(&self) -> usize {
        self.graphemes(true).count()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // Safety: bounds have already been checked in new_with_bounds / with_bounds
        unsafe { self.string.get_unchecked(self.bounds.clone()) }
    }
}

impl PartialEq for ValueString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for ValueString {}

impl Hash for ValueString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Deref for ValueString {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for ValueString {
    fn from(s: &str) -> Self {
        Self::new(s.into())
    }
}

impl From<String> for ValueString {
    fn from(s: String) -> Self {
        Self::new(s.into())
    }
}

impl fmt::Debug for ValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ValueString(bounds: {:?}, string: \"{}\")",
            self.bounds,
            self.as_str()
        )
    }
}

impl fmt::Display for ValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "\"{}\"", self.as_str())
        } else {
            write!(f, "{}", self.as_str())
        }
    }
}

thread_local!(
    static EMPTY_STRING: Rc<str> = Rc::from("");
);
