use crate::{prelude::*, Ptr, Result};
use koto_parser::StringSlice;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
};
use unicode_segmentation::UnicodeSegmentation;

/// The String type used by the Koto runtime
///
/// The underlying string data is shared between instances, with internal bounds allowing for shared
/// subslices.
///
/// [`AsRef`](std::convert::AsRef) is implemented for &str, which automatically resolves to the
/// correct slice of the string data.
#[derive(Clone)]
pub struct KString(Inner);

// Either the full string, or a slice
//
// By heap-allocating slice bounds we can keep KString's size down to 16 bytes; otherwise it
// would have a size of 32 bytes.
#[derive(Clone)]
enum Inner {
    Full(Ptr<str>),
    Slice(Ptr<StringSlice>),
}

impl KString {
    /// Returns the empty string
    ///
    /// This returns a clone of an empty KString which is initialized once per thread.
    pub fn empty() -> Self {
        Self::from(EMPTY_STRING.with(|s| s.clone()))
    }

    /// Initializes a new KString with the provided data and bounds
    ///
    /// If the bounds aren't valid for the data then `None` is returned.
    pub fn new_with_bounds(string: Ptr<str>, bounds: Range<usize>) -> Option<Self> {
        StringSlice::new(string, bounds).map(Self::from)
    }

    /// Returns a new KString with shared data and new bounds
    ///
    /// If the bounds aren't valid for the string then `None` is returned.
    pub fn with_bounds(&self, new_bounds: Range<usize>) -> Option<Self> {
        let slice = match &self.0 {
            Inner::Full(string) => StringSlice::from(string.clone()),
            Inner::Slice(slice) => slice.deref().clone(),
        };

        slice.with_bounds(new_bounds).map(Self::from)
    }

    /// Returns a new KString with shared data and bounds defined by the grapheme indices
    ///
    /// This allows for subslicing by index, with the index referring to unicode graphemes.
    ///
    /// If the provided indices are out of bounds then an empty string will be returned.
    pub fn with_grapheme_indices(&self, indices: Range<usize>) -> Self {
        let start = indices.start;
        let end = indices.end;

        if start == end {
            return Self::empty();
        }

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
            }

            if i == end - 1 {
                // Checking against end - 1 in the same way as for result_start,
                // allowing for indexing one-past-the-end.
                // e.g. assert_eq 'xyz'[1..3], 'yz'
                result_end = Some(grapheme_start + grapheme.len());
                break;
            }
        }

        let result_bounds = match (result_start, result_end) {
            (Some(result_start), Some(result_end)) => result_start..result_end,
            (Some(result_start), None) => result_start..self.len(),
            _ => return Self::empty(),
        };

        self.with_bounds(result_bounds).unwrap_or_else(Self::empty)
    }

    /// Removes and returns the first grapheme from the string
    pub fn pop_front(&mut self) -> Option<Self> {
        match self.clone().graphemes(true).next() {
            Some(grapheme) => match &mut self.0 {
                Inner::Full(string) => {
                    let (popped, rest) = StringSlice::from(string.clone())
                        .split(grapheme.len())
                        .unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (popped, rest) = slice.split(grapheme.len()).unwrap();
                    *Ptr::make_mut(slice) = rest;
                    Some(popped.into())
                }
            },
            None => None,
        }
    }

    /// Removes and returns the last grapheme from the string
    pub fn pop_back(&mut self) -> Option<Self> {
        match self.clone().graphemes(true).next_back() {
            Some(grapheme) => match &mut self.0 {
                Inner::Full(string) => {
                    let (rest, popped) = StringSlice::from(string.clone())
                        .split(string.len() - grapheme.len())
                        .unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (rest, popped) =
                        slice.split(slice.as_str().len() - grapheme.len()).unwrap();
                    *Ptr::make_mut(slice) = rest;
                    Some(popped.into())
                }
            },
            None => None,
        }
    }

    /// Returns the number of graphemes contained within the KString's bounds
    pub fn grapheme_count(&self) -> usize {
        self.graphemes(true).count()
    }

    /// Returns the `&str` within the KString's bounds
    pub fn as_str(&self) -> &str {
        match &self.0 {
            Inner::Full(string) => string,
            Inner::Slice(slice) => slice.as_str(),
        }
    }

    /// Renders the string to the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        if ctx.is_contained() {
            ctx.append('\'');
            ctx.append(self);
            ctx.append('\'');
        } else {
            ctx.append(self);
        }
        Ok(())
    }
}

impl PartialEq<&str> for KString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq for KString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for KString {}

impl Hash for KString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Deref for KString {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for KString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<Ptr<str>> for KString {
    fn from(string: Ptr<str>) -> Self {
        Self(Inner::Full(string))
    }
}

impl From<StringSlice> for KString {
    fn from(slice: StringSlice) -> Self {
        Self(Inner::Slice(slice.into()))
    }
}

impl From<String> for KString {
    fn from(s: String) -> Self {
        Self::from(Ptr::<str>::from(s.into_boxed_str()))
    }
}

impl From<&str> for KString {
    fn from(s: &str) -> Self {
        Self::from(Ptr::<str>::from(s))
    }
}

impl fmt::Display for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

thread_local!(
    static EMPTY_STRING: Ptr<str> = Ptr::from("");
);
