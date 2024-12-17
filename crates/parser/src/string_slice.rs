use koto_memory::Ptr;
use std::ops::{Deref, Range};

/// String data with 32-bit bounds
///
/// The bounds are guaranteed to be indices to a valid UTF-8 sub-string of the original data.
#[derive(Clone, Debug)]
pub struct StringSlice {
    data: Ptr<String>,
    bounds: Range<u32>,
}

impl StringSlice {
    /// Initializes a string slice with the given string data and bounds
    ///
    /// If the bounds aren't valid for the given string data then None is returned.
    pub fn new(string: Ptr<String>, bounds: Range<usize>) -> Option<Self> {
        Self::try_from(string)
            .ok()
            .and_then(|s| s.with_bounds(bounds))
    }

    /// Initializes a string slice with the given string data and bounds
    ///
    /// # Safety
    /// Care must be taken to ensure that the bounds are valid within the provided string,
    /// i.e. `string.get(bounds).is_some()` must be true.
    pub unsafe fn new_unchecked(string: Ptr<String>, bounds: Range<u32>) -> Self {
        Self {
            data: string,
            bounds,
        }
    }

    /// Returns a new string slice with shared data and new bounds
    ///
    /// If the bounds aren't valid within the current string slice, then None is returned.
    pub fn with_bounds(&self, bounds: Range<usize>) -> Option<Self> {
        let new_bounds =
            (bounds.start + self.bounds.start as usize)..(bounds.end + self.bounds.start as usize);

        if self.data.get(new_bounds.clone()).is_some() {
            Some(Self {
                data: self.data.clone(),
                bounds: usize_to_u32_range(&new_bounds),
            })
        } else {
            None
        }
    }

    /// Returns the string slice as a `&str`
    pub fn as_str(&self) -> &str {
        // Safety: bounds have already been checked in new_with_bounds / with_bounds
        unsafe { self.data.get_unchecked(u32_to_usize_range(&self.bounds)) }
    }

    /// Splits the string slice at the given byte offset, returning the two resulting strings
    ///
    /// If the offset is outside of the string slice's bounds or would produce invalid UTF-8 data,
    /// then None is returned.
    pub fn split(&self, offset: usize) -> Option<(Self, Self)> {
        if self.as_str().is_char_boundary(offset) {
            let split_point = self.bounds.start + offset as u32;
            Some((
                Self {
                    data: self.data.clone(),
                    bounds: self.bounds.start..split_point,
                },
                Self {
                    data: self.data.clone(),
                    bounds: split_point..self.bounds.end,
                },
            ))
        } else {
            None
        }
    }
}

impl TryFrom<Ptr<String>> for StringSlice {
    type Error = Ptr<String>;

    fn try_from(string: Ptr<String>) -> std::result::Result<Self, Self::Error> {
        u32::try_from(string.len())
            .map(|len| Self {
                data: string.clone(),
                bounds: 0_u32..len,
            })
            .map_err(|_| string)
    }
}

impl<'a> TryFrom<&'a str> for StringSlice {
    type Error = &'a str;

    fn try_from(s: &'a str) -> std::result::Result<Self, Self::Error> {
        u32::try_from(s.len())
            .map(|len| Self {
                data: s.to_string().into(),
                bounds: 0_u32..len,
            })
            .map_err(|_| s)
    }
}

impl Deref for StringSlice {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for StringSlice {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl PartialEq<StringSlice> for StringSlice {
    fn eq(&self, other: &StringSlice) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for StringSlice {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

pub(crate) fn u32_to_usize_range(r: &Range<u32>) -> Range<usize> {
    r.start as usize..r.end as usize
}

pub(crate) fn usize_to_u32_range(r: &Range<usize>) -> Range<u32> {
    r.start as u32..r.end as u32
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let string = Ptr::from("abcdef".to_string());
        let slice = StringSlice::new(string, 1..3).unwrap();
        assert_eq!(slice.as_str(), "bc");
    }

    #[test]
    fn with_bounds() {
        let original = StringSlice::try_from("0123456789").unwrap();
        let slice = original.with_bounds(4..8).unwrap();
        assert_eq!(slice.as_str(), "4567");
    }

    #[test]
    fn split() {
        let original = StringSlice::try_from("hello, world!").unwrap();
        let (a, b) = original.split(6).unwrap();
        assert_eq!(a.as_str(), "hello,");
        assert_eq!(b.as_str(), " world!");
    }

    #[test]
    fn equality() {
        let s1 = StringSlice::try_from("abc").unwrap();
        let s2 = StringSlice::try_from("xyz").unwrap();
        let s3 = StringSlice::new(Ptr::from("___xyz___".to_string()), 3..6).unwrap();
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
        assert_eq!(s2, s3);
        assert_eq!(s2, "xyz");
        assert_eq!(s3, "xyz");
    }
}
