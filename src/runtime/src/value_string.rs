use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, Range},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct ValueString {
    string: Arc<str>,
    bounds: Option<Range<usize>>,
}

impl ValueString {
    fn new(string: Arc<str>) -> Self {
        Self {
            string,
            bounds: None,
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.bounds {
            Some(bounds) => &self.string[bounds.clone()],
            None => &self.string,
        }
    }

    pub fn with_bounds(&self, new_bounds: Range<usize>) -> Self {
        let bounds = match &self.bounds {
            Some(bounds) => (bounds.start + new_bounds.start)..(bounds.start + new_bounds.end),
            None => new_bounds,
        };
        Self {
            string: self.string.clone(),
            bounds: Some(bounds),
        }
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

impl fmt::Display for ValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}
