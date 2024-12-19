use koto_memory::Ptr;
use std::ops::{Deref, Range};

/// A trait for index types used by [StringSlice]
pub trait StringSliceIndex: TryFrom<usize> + Copy + Default {
    // u16 doesn't implement Into<usize>
    fn to_usize(self) -> usize;
}

impl StringSliceIndex for usize {
    fn to_usize(self) -> usize {
        self
    }
}

impl StringSliceIndex for u16 {
    fn to_usize(self) -> usize {
        self as usize
    }
}

/// String data with defined bounds
///
/// The bounds are guaranteed to be indices to a valid UTF-8 sub-string of the original data.
#[derive(Clone, Debug)]
pub struct StringSlice<T> {
    data: Ptr<String>,
    bounds: Range<T>,
    _niche: bool,
}

impl<T> StringSlice<T>
where
    T: StringSliceIndex,
{
    /// Initializes a string slice with the given string data and bounds
    ///
    /// If the bounds aren't valid for the given string data then None is returned.
    pub fn new(string: Ptr<String>, bounds: Range<usize>) -> Option<Self> {
        try_from_range(&bounds).map(|bounds| Self {
            data: string,
            bounds,
            _niche: false,
        })
    }

    /// Initializes a string slice with the given string data and bounds
    ///
    /// # Safety
    /// Care must be taken to ensure that the bounds are valid within the provided string,
    /// i.e. `string.get(bounds).is_some()` must be true.
    pub unsafe fn new_unchecked(string: Ptr<String>, bounds: Range<T>) -> Self {
        Self {
            data: string,
            bounds,
            _niche: false,
        }
    }

    /// Returns a new string slice with shared data and new bounds
    ///
    /// If the bounds aren't valid within the current string slice, then None is returned.
    pub fn with_bounds(&self, bounds: Range<usize>) -> Option<Self> {
        let new_bounds = (bounds.start + self.bounds.start.to_usize())
            ..(bounds.end + self.bounds.start.to_usize());

        if self.data.get(new_bounds.clone()).is_some() {
            try_from_range(&new_bounds).map(|bounds| Self {
                data: self.data.clone(),
                bounds,
                _niche: false,
            })
        } else {
            None
        }
    }

    /// Attempt to convert this slice into a slice with an alternative bound type
    ///
    /// Currently this is only used to try to convert `usize` bounds into `u32`.
    pub fn try_convert<U>(&self) -> Option<StringSlice<U>>
    where
        U: TryFrom<T>,
    {
        try_from_range(&self.bounds).map(|bounds| StringSlice::<U> {
            data: self.data.clone(),
            bounds,
            _niche: false,
        })
    }

    /// Returns the string slice as a `&str`
    pub fn as_str(&self) -> &str {
        // Safety: bounds have already been checked in new_with_bounds / with_bounds
        unsafe { self.data.get_unchecked(to_usize_range(&self.bounds)) }
    }

    /// Splits the string slice at the given byte offset, returning the two resulting strings
    ///
    /// If the offset is outside of the string slice's bounds or would produce invalid UTF-8 data,
    /// then None is returned.
    pub fn split(&self, offset: usize) -> Option<(Self, Self)> {
        let split_point = self.bounds.start.to_usize() + offset;
        if self.data.is_char_boundary(split_point) {
            if let Ok(split_point_t) = T::try_from(split_point) {
                Some((
                    Self {
                        data: self.data.clone(),
                        bounds: self.bounds.start..split_point_t,
                        _niche: false,
                    },
                    Self {
                        data: self.data.clone(),
                        bounds: split_point_t..self.bounds.end,
                        _niche: false,
                    },
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl From<Ptr<String>> for StringSlice<usize> {
    fn from(string: Ptr<String>) -> Self {
        let bounds = 0..string.len();
        Self {
            data: string,
            bounds,
            _niche: false,
        }
    }
}

impl From<&str> for StringSlice<usize> {
    fn from(string: &str) -> Self {
        let bounds = 0..string.len();
        Self {
            data: string.to_string().into(),
            bounds,
            _niche: false,
        }
    }
}

impl<T> Deref for StringSlice<T>
where
    T: StringSliceIndex,
{
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<T> AsRef<str> for StringSlice<T>
where
    T: StringSliceIndex,
{
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl<T> PartialEq<StringSlice<T>> for StringSlice<T>
where
    T: StringSliceIndex,
{
    fn eq(&self, other: &StringSlice<T>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<T> PartialEq<&str> for StringSlice<T>
where
    T: StringSliceIndex,
{
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

fn to_usize_range<T>(r: &Range<T>) -> Range<usize>
where
    T: StringSliceIndex,
{
    r.start.to_usize()..r.end.to_usize()
}

fn try_from_range<T, U>(r: &Range<U>) -> Option<Range<T>>
where
    T: TryFrom<U>,
    U: Copy,
{
    match (T::try_from(r.start), T::try_from(r.end)) {
        (Ok(start), Ok(end)) => Some(start..end),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let string = Ptr::from("abcdef".to_string());
        let slice = StringSlice::<usize>::new(string, 1..3).unwrap();
        assert_eq!(slice.as_str(), "bc");
    }

    #[test]
    fn with_bounds() {
        let original = StringSlice::from("0123456789");
        let slice = original.with_bounds(4..8).unwrap();
        assert_eq!(slice.as_str(), "4567");
    }

    #[test]
    fn split() {
        let original = StringSlice::<usize>::try_from("hello, world!").unwrap();
        let (a, b) = original.split(6).unwrap();
        assert_eq!(a.as_str(), "hello,");
        assert_eq!(b.as_str(), " world!");
    }

    #[test]
    fn equality() {
        let s1 = StringSlice::from("abc");
        let s2 = StringSlice::from("xyz");
        let s3 = StringSlice::new(Ptr::from("___xyz___".to_string()), 3..6).unwrap();
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
        assert_eq!(s2, s3);
        assert_eq!(s2, "xyz");
        assert_eq!(s3, "xyz");
    }
}
