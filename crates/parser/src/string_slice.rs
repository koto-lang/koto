use koto_memory::Ptr;
use std::ops::{Deref, Range};

trait StringSliceIndex: TryFrom<usize> + Copy + Default {
    // u32 doesn't implement Into<usize>
    fn to_usize(self) -> usize;
}

impl StringSliceIndex for usize {
    fn to_usize(self) -> usize {
        self
    }
}
impl StringSliceIndex for u32 {
    fn to_usize(self) -> usize {
        self as usize
    }
}

/// String data with 32-bit bounds
///
/// The bounds are guaranteed to be indices to a valid UTF-8 sub-string of the original data.
#[derive(Clone, Debug)]
pub struct StringSlice<T> {
    data: Ptr<String>,
    bounds: Range<T>,
}

impl<T> StringSlice<T>
where
    T: StringSliceIndex,
{
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
    pub unsafe fn new_unchecked(string: Ptr<String>, bounds: Range<T>) -> Self {
        Self {
            data: string,
            bounds,
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
            })
        } else {
            None
        }
    }

    pub fn try_convert<U>(&self) -> Option<StringSlice<U>>
    where
        U: TryFrom<T>,
    {
        try_from_range(&self.bounds).map(|bounds| StringSlice::<U> {
            data: self.data.clone(),
            bounds,
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
                    },
                    Self {
                        data: self.data.clone(),
                        bounds: split_point_t..self.bounds.end,
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

impl<T> TryFrom<Ptr<String>> for StringSlice<T>
where
    T: StringSliceIndex,
{
    type Error = Ptr<String>;

    fn try_from(string: Ptr<String>) -> std::result::Result<Self, Self::Error> {
        T::try_from(string.len())
            .map(|len| Self {
                data: string.clone(),
                bounds: T::default()..len,
            })
            .map_err(|_| string)
    }
}

impl<'a, T> TryFrom<&'a str> for StringSlice<T>
where
    T: StringSliceIndex,
{
    type Error = &'a str;

    fn try_from(s: &'a str) -> std::result::Result<Self, Self::Error> {
        T::try_from(s.len())
            .map(|len| Self {
                data: s.to_string().into(),
                bounds: T::default()..len,
            })
            .map_err(|_| s)
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

pub(crate) fn to_usize_range<T>(r: &Range<T>) -> Range<usize>
where
    T: StringSliceIndex,
{
    r.start.to_usize()..r.end.to_usize()
}

pub(crate) fn try_from_range<T, U>(r: &Range<U>) -> Option<Range<T>>
where
    T: TryFrom<U>,
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
        let slice = StringSlice::new(string, 1..3).unwrap();
        assert_eq!(slice.as_str(), "bc");
    }

    #[test]
    fn with_bounds() {
        let original = StringSlice::<u32>::try_from("0123456789").unwrap();
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
