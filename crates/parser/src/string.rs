use crate::StringSlice;
use koto_memory::Ptr;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    path::{Path, PathBuf},
};
use unicode_segmentation::UnicodeSegmentation;

/// The String type used by the Koto runtime
///
/// The underlying string data is shared between instances, with internal bounds allowing for shared
/// subslices.
///
/// [`AsRef`](std::convert::AsRef) is implemented for `&str`, which automatically resolves to the
/// correct slice of the string data.
#[derive(Clone)]
pub struct KString(Inner);

#[derive(Clone)]
enum Inner {
    // A shared string
    Full(Ptr<String>),
    // A shared string with 16bit bounds, small enough to store without extra allocation
    Slice(StringSlice<u16>),
    // A shared string with bounds, heap-allocated to keep the size of `KString` down to 24 bytes
    SliceLarge(Ptr<StringSlice<usize>>),
}

impl KString {
    /// Returns a new KString with shared data and new bounds
    ///
    /// If the bounds aren't valid for the string then `None` is returned.
    pub fn with_bounds(&self, new_bounds: Range<usize>) -> Option<Self> {
        match &self.0 {
            Inner::Full(string) => {
                StringSlice::<usize>::new(string.clone(), new_bounds).map(Self::from)
            }
            Inner::Slice(slice) => slice.with_bounds(new_bounds).map(Self::from),
            Inner::SliceLarge(slice) => slice.with_bounds(new_bounds).map(Self::from),
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
                Inner::Full(string) => {
                    let slice = StringSlice::<usize>::from(string.clone());
                    let (popped, rest) = slice.split(grapheme.len()).unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (popped, rest) = slice.split(grapheme.len()).unwrap();
                    *slice = rest;
                    Some(popped.into())
                }
                Inner::SliceLarge(slice) => {
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
    /// and pop characters without introducing extra allocations (assuming the bounds are within 16
    /// bit limits, otherwise an allocated slice will be used when ).
    pub fn pop_back(&mut self) -> Option<Self> {
        match self.clone().graphemes(true).next_back() {
            Some(grapheme) => match &mut self.0 {
                Inner::Full(string) => {
                    let slice = StringSlice::<usize>::from(string.clone());
                    let (rest, popped) = slice.split(string.len() - grapheme.len()).unwrap();
                    *self = rest.into();
                    Some(popped.into())
                }
                Inner::Slice(slice) => {
                    let (rest, popped) = slice.split(slice.len() - grapheme.len()).unwrap();
                    *slice = rest;
                    Some(popped.into())
                }
                Inner::SliceLarge(slice) => {
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
            Inner::Full(string) => string,
            Inner::Slice(slice) => slice,
            Inner::SliceLarge(slice) => slice,
        }
    }
}

impl From<Ptr<String>> for KString {
    fn from(string: Ptr<String>) -> Self {
        Self(Inner::Full(string))
    }
}

impl From<StringSlice<usize>> for KString {
    fn from(slice: StringSlice<usize>) -> Self {
        if let Some(slice16) = slice.try_convert() {
            Self(Inner::Slice(slice16))
        } else {
            Self(Inner::SliceLarge(slice.into()))
        }
    }
}

impl From<StringSlice<u16>> for KString {
    fn from(slice: StringSlice<u16>) -> Self {
        Self(Inner::Slice(slice))
    }
}

impl From<String> for KString {
    fn from(s: String) -> Self {
        Ptr::<String>::from(s).into()
    }
}

impl From<&str> for KString {
    fn from(s: &str) -> Self {
        Ptr::<String>::from(s.to_string()).into()
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
