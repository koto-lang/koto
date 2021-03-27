use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    sync::Arc,
};

#[derive(Clone)]
pub struct ValueString {
    string: Arc<str>,
    bounds: Range<usize>,
}

impl ValueString {
    fn new(string: Arc<str>) -> Self {
        let bounds = 0..string.len();
        Self { string, bounds }
    }

    pub fn new_with_bounds(string: Arc<str>, bounds: Range<usize>) -> Result<Self, String> {
        if string.get(bounds.clone()).is_some() {
            Ok(Self { string, bounds })
        } else {
            Err("new_with_bounds: Invalid bounds".to_string())
        }
    }

    pub fn with_bounds(&self, mut new_bounds: Range<usize>) -> Result<Self, String> {
        new_bounds.end += self.bounds.start;
        new_bounds.start += self.bounds.start;

        if new_bounds.end <= self.bounds.end && self.string.get(new_bounds.clone()).is_some() {
            Ok(Self {
                string: self.string.clone(),
                bounds: new_bounds,
            })
        } else {
            Err("with_bounds: Invalid bounds".to_string())
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // Safety: bounds have already been checked in new_with_bounds / with_bounds
        unsafe { &self.string.get_unchecked(self.bounds.clone()) }
    }
}

impl PartialEq for ValueString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

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
