use std::fmt;

use crate::ValueString;

/// A helper for building strings
#[derive(Debug, Clone, Default)]
pub struct StringBuilder {
    string: String,
}

impl StringBuilder {
    /// Makes a new string builder with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            string: String::with_capacity(capacity),
        }
    }

    /// Appends a value to the end of the string
    pub fn append<'a>(&mut self, value: impl Into<StringBuilderAppend<'a>>) {
        value.into().append(&mut self.string)
    }

    /// Returns the built string, consuming the builder
    pub fn build(self) -> String {
        self.string
    }
}

impl fmt::Write for StringBuilder {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.append(s);
        Ok(())
    }
}

/// Types that can be appended to [StringBuilder]
pub enum StringBuilderAppend<'a> {
    Char(char),
    Str(&'a str),
    String(String),
    ValueString(ValueString),
    ValueStringRef(&'a ValueString),
}

impl From<char> for StringBuilderAppend<'_> {
    fn from(value: char) -> Self {
        StringBuilderAppend::Char(value)
    }
}

impl<'a> From<&'a str> for StringBuilderAppend<'a> {
    fn from(value: &'a str) -> Self {
        StringBuilderAppend::Str(value)
    }
}

impl From<String> for StringBuilderAppend<'_> {
    fn from(value: String) -> Self {
        StringBuilderAppend::String(value)
    }
}

impl From<ValueString> for StringBuilderAppend<'_> {
    fn from(value: ValueString) -> Self {
        StringBuilderAppend::ValueString(value)
    }
}

impl<'a> From<&'a ValueString> for StringBuilderAppend<'a> {
    fn from(value: &'a ValueString) -> Self {
        StringBuilderAppend::ValueStringRef(value)
    }
}

impl<'a> StringBuilderAppend<'a> {
    fn append(self, string: &mut String) {
        match self {
            StringBuilderAppend::Char(c) => string.push(c),
            StringBuilderAppend::Str(s) => string.push_str(s),
            StringBuilderAppend::String(s) => string.push_str(&s),
            StringBuilderAppend::ValueString(s) => string.push_str(&s),
            StringBuilderAppend::ValueStringRef(s) => string.push_str(s),
        }
    }
}
