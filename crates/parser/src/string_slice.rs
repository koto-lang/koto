use koto_memory::Ptr;
use std::ops::Range;

/// String data with bounds
///
/// The bounds are guaranteed to be indices to a valid UTF-8 sub-string of the original data.
#[derive(Clone, Debug)]
pub struct StringSlice {
    data: Ptr<str>,
    bounds: Range<usize>,
}

impl StringSlice {
    /// Initalizes a string slice with the given string data and bounds
    ///
    /// If the bounds aren't valid for the given string data then None is returned.
    pub fn new(string: Ptr<str>, bounds: Range<usize>) -> Option<Self> {
        if string.get(bounds.clone()).is_some() {
            Some(Self {
                data: string,
                bounds,
            })
        } else {
            None
        }
    }

    /// Initalizes a string slice with the given string data and bounds
    ///
    /// # Safety
    /// Care must be taken to ensure that the bounds are valid within the provided string,
    /// i.e. string.get(bounds).is_some() must be true.
    pub unsafe fn new_unchecked(string: Ptr<str>, bounds: Range<usize>) -> Self {
        Self {
            data: string,
            bounds,
        }
    }

    /// Returns a new string slice with shared data and new bounds
    ///
    /// If the bounds aren't valid within the current string slice, then None is returned.
    pub fn with_bounds(&self, bounds: Range<usize>) -> Option<Self> {
        let new_bounds = (bounds.start + self.bounds.start)..(bounds.end + self.bounds.start);

        if self.data.get(new_bounds.clone()).is_some() {
            Some(Self {
                data: self.data.clone(),
                bounds: new_bounds,
            })
        } else {
            None
        }
    }

    /// Returns the string slice as a `&str`
    pub fn as_str(&self) -> &str {
        // Safety: bounds have already been checked in new_with_bounds / with_bounds
        unsafe { self.data.get_unchecked(self.bounds.clone()) }
    }

    /// Splits the string slice at the given byte offset, returning the two resulting strings
    ///
    /// If the offset is outside of the string slice's bounds or would produce invalid UTF-8 data,
    /// then None is returned.
    pub fn split(&self, offset: usize) -> Option<(Self, Self)> {
        if self.as_str().is_char_boundary(offset) {
            let split_point = self.bounds.start + offset;
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

impl From<Ptr<str>> for StringSlice {
    fn from(string: Ptr<str>) -> Self {
        let bounds = 0..string.len();
        Self {
            data: string,
            bounds,
        }
    }
}

impl From<String> for StringSlice {
    fn from(string: String) -> Self {
        Self::from(Ptr::from(string))
    }
}

impl AsRef<str> for StringSlice {
    fn as_ref(&self) -> &str {
        self.as_str()
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
        let original = StringSlice::from("0123456789".to_string());
        let slice = original.with_bounds(4..8).unwrap();
        assert_eq!(slice.as_str(), "4567");
    }

    #[test]
    fn split() {
        let original = StringSlice::from(Ptr::from("hello, world!".to_string()));
        let (a, b) = original.split(6).unwrap();
        assert_eq!(a.as_str(), "hello,");
        assert_eq!(b.as_str(), " world!");
    }

    #[test]
    fn equality() {
        let s1 = StringSlice::from(Ptr::from("abc".to_string()));
        let s2 = StringSlice::from(Ptr::from("xyz".to_string()));
        let s3 = StringSlice::new(Ptr::from("___xyz___".to_string()), 3..6).unwrap();
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
        assert_eq!(s2, s3);
        assert_eq!(s2, "xyz");
        assert_eq!(s3, "xyz");
    }
}
