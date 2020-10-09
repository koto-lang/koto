use std::{fmt, ops::Deref, sync::Arc};

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ValueString {
    string: Arc<str>,
}

impl ValueString {
    fn new(string: Arc<str>) -> Self {
        Self { string }
    }

    pub fn as_str(&self) -> &str {
        self
    }
}

impl Deref for ValueString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.string
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
