use crate::StringSlice;
use koto_memory::Ptr;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    path::{Path, PathBuf},
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
    // A string that's short enough to be stored inline without extra allocation
    Inline(InlineString),
    // A shared heap-allocated string
    Full(Ptr<String>),
    // A shared heap-allocated string with bounds
    Slice(StringSlice),
}

impl KString {
    /// Returns a new KString with shared data and new bounds
    ///
    /// If the bounds aren't valid for the string then `None` is returned.
    pub fn with_bounds(&self, new_bounds: Range<usize>) -> Option<Self> {
        match &self.0 {
            Inner::Inline(inline) => return inline.with_bounds(new_bounds).map(Self::from),
            Inner::Full(string) => StringSlice::new(string.clone(), new_bounds).map(Self::from),
            Inner::Slice(slice) => slice.with_bounds(new_bounds).map(Self::from),
        }
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
                    let Ok(slice) = StringSlice::try_from(string.clone()) else {
                        return None;
                    };
                    let (popped, rest) = slice.split(grapheme.len()).unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (popped, rest) = slice.split(grapheme.len()).unwrap();
                    *slice = rest;
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
                    let Ok(slice) = StringSlice::try_from(string.clone()) else {
                        return None;
                    };
                    let (rest, popped) = slice.split(string.len() - grapheme.len()).unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (rest, popped) = slice.split(slice.len() - grapheme.len()).unwrap();
                    *slice = rest;
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

impl From<Ptr<String>> for KString {
    fn from(string: Ptr<String>) -> Self {
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
            Err(()) => Ptr::<String>::from(s).into(),
        }
    }
}

impl From<&str> for KString {
    fn from(s: &str) -> Self {
        match InlineString::try_from(s) {
            Ok(inline) => inline.into(),
            Err(()) => Ptr::<String>::from(s.to_string()).into(),
        }
    }
}

impl From<PathBuf> for KString {
    fn from(path: PathBuf) -> Self {
        Self::from(path.to_string_lossy().to_string())
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

impl AsRef<Path> for KString {
    fn as_ref(&self) -> &Path {
        Path::new(self.as_str())
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
        self.as_str().get(bounds).map(Self::from_short_string)
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
