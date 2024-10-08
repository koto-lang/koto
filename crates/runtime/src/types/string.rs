use crate::{prelude::*, Ptr, Result};
use koto_parser::StringSlice;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    str::from_utf8_unchecked,
};
use unicode_segmentation::UnicodeSegmentation;

/// The String type used by the Koto runtime
///
/// For heap allocated strings, the underlying string data is shared between instances,
/// with internal bounds allowing for shared subslices.
///
/// [`AsRef`](std::convert::AsRef) is implemented for `&str`, which automatically resolves to the
/// correct slice of the string data.
#[derive(Clone)]
pub struct KString(Inner);

#[derive(Clone)]
enum Inner {
    // A string that's short enough to be stored without allocation
    Inline(InlineString),
    // A heap-allocated string
    Full(Ptr<str>),
    // A heap-allocated string with bounds
    //
    // By heap-allocating the slice bounds we can keep the size of KString below 24 bytes,
    // which is the maximum allowed by KValue.
    Slice(Ptr<StringSlice>),
}

impl KString {
    /// Returns a new KString with shared data and new bounds
    ///
    /// If the bounds aren't valid for the string then `None` is returned.
    pub fn with_bounds(&self, new_bounds: Range<usize>) -> Option<Self> {
        let slice = match &self.0 {
            Inner::Inline(inline) => return inline.with_bounds(new_bounds).map(Self::from),
            Inner::Full(string) => StringSlice::from(string.clone()),
            Inner::Slice(slice) => slice.deref().clone(),
        };

        slice.with_bounds(new_bounds).map(Self::from)
    }

    /// Returns a new KString with shared data and bounds defined by the grapheme indices
    ///
    /// This allows for subslicing by index, with the index referring to Unicode graphemes.
    ///
    /// If the provided indices are out of bounds then an empty string will be returned.
    pub fn with_grapheme_indices(&self, indices: Range<usize>) -> Self {
        let start = indices.start;
        let end = indices.end;

        if start == end {
            return Self::default();
        }

        let mut result_start = if start == 0 { Some(0) } else { None };
        let mut result_end = None;

        for (i, (grapheme_start, grapheme)) in self.grapheme_indices(true).enumerate() {
            if result_start.is_none() && i == start - 1 {
                // By checking against start - 1 (rather than waiting until the next iteration),
                // we can allow for indexing from 'one past the end' to get to an empty string,
                // which can be useful when consuming characters from a string.
                // E.g.
                //   x = get_string()
                //   do_something_with_first_char x[0]
                //   do_something_with_remaining_string x[1..]
                result_start = Some(grapheme_start + grapheme.len());
            }

            if i == end - 1 {
                // Checking against end - 1 in the same way as for result_start,
                // allowing for indexing one-past-the-end.
                // E.g. `assert_eq 'xyz'[1..3], 'yz'`
                result_end = Some(grapheme_start + grapheme.len());
                break;
            }
        }

        let result_bounds = match (result_start, result_end) {
            (Some(result_start), Some(result_end)) => result_start..result_end,
            (Some(result_start), None) => result_start..self.len(),
            _ => return Self::default(),
        };

        self.with_bounds(result_bounds).unwrap_or_default()
    }

    /// Removes and returns the first grapheme from the string
    ///
    /// Although strings are treated as immutable in Koto scripts, there are cases where it's useful
    /// to be able to mutate the string data in place. For example, iterators can hold on to a string
    /// and pop characters without introducing extra allocations.
    pub fn pop_front(&mut self) -> Option<Self> {
        match self.clone().graphemes(true).next() {
            Some(grapheme) => match &mut self.0 {
                Inner::Inline(inline) => {
                    let (popped, rest) = inline.split(grapheme.len()).unwrap();
                    *inline = rest;
                    Some(popped.into())
                }
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
    ///
    /// Although strings are treated as immutable in Koto scripts, there are cases where it's useful
    /// to be able to mutate the string data in place. For example, iterators can hold on to a string
    /// and pop characters without introducing extra allocations.
    pub fn pop_back(&mut self) -> Option<Self> {
        match self.clone().graphemes(true).next_back() {
            Some(grapheme) => match &mut self.0 {
                Inner::Inline(inline) => {
                    let (rest, popped) = inline.split(inline.len() - grapheme.len()).unwrap();
                    *inline = rest;
                    Some(popped.into())
                }
                Inner::Full(string) => {
                    let (rest, popped) = StringSlice::from(string.clone())
                        .split(string.len() - grapheme.len())
                        .unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (rest, popped) = slice.split(slice.len() - grapheme.len()).unwrap();
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
            Inner::Inline(inline) => inline,
            Inner::Full(string) => string,
            Inner::Slice(slice) => slice,
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

impl Default for KString {
    fn default() -> Self {
        InlineString::default().into()
    }
}

impl From<InlineString> for KString {
    fn from(string: InlineString) -> Self {
        Self(Inner::Inline(string))
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
        match InlineString::try_from(s.as_str()) {
            Ok(inline) => inline.into(),
            Err(()) => Ptr::<str>::from(s.into_boxed_str()).into(),
        }
    }
}

impl From<&str> for KString {
    fn from(s: &str) -> Self {
        match InlineString::try_from(s) {
            Ok(inline) => inline.into(),
            Err(()) => Ptr::<str>::from(s).into(),
        }
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
        self.deref()
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

// Q. Why 22?
// A. KValue has a maximum variant size of 24, -1 for the inline string len, and -1_for the KString
//    variant tag.
const MAX_INLINE_STRING_LEN: usize = 22;

#[derive(Clone, Default)]
struct InlineString {
    data: [u8; MAX_INLINE_STRING_LEN],
    len: u8,
}

impl InlineString {
    // `string` must have a length less than or equal to `MAX_INLINE_STRING_LEN`
    fn from_short_string(string: &str) -> Self {
        let len = string.len();
        debug_assert!(len <= MAX_INLINE_STRING_LEN);

        let mut result = InlineString {
            data: Default::default(),
            len: len as u8,
        };
        result.data[..len].copy_from_slice(string.as_bytes());

        result
    }

    fn with_bounds(&self, bounds: Range<usize>) -> Option<Self> {
        self.as_str()
            .get(bounds.clone())
            .map(Self::from_short_string)
    }

    fn split(&self, split_point: usize) -> Option<(Self, Self)> {
        let s = self.as_str();
        if s.is_char_boundary(split_point) {
            Some((
                Self::from_short_string(&s[..split_point]),
                Self::from_short_string(&s[split_point..]),
            ))
        } else {
            None
        }
    }

    fn as_str(&self) -> &str {
        // Safety: the data and len were guaranteed to be valid UTF-8 in every initializer
        unsafe { from_utf8_unchecked(&self.data[..self.len as usize]) }
    }
}

impl TryFrom<&str> for InlineString {
    type Error = ();

    fn try_from(s: &str) -> std::result::Result<Self, Self::Error> {
        if s.len() <= MAX_INLINE_STRING_LEN {
            Ok(Self::from_short_string(s))
        } else {
            Err(())
        }
    }
}

impl Deref for InlineString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
