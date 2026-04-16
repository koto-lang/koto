use crate::{KString, KotoVm};
use std::fmt;

/// A helper for converting Koto values to strings.
#[derive(Default)]
pub struct DisplayContext<'a> {
    result: String,
    vm: Option<&'a KotoVm>,
    parent_containers: Vec<usize>,
    debug: bool,
}

impl<'a> DisplayContext<'a> {
    /// Makes a display context with the given VM.
    pub fn with_vm(vm: &'a KotoVm) -> Self {
        Self {
            result: String::default(),
            vm: Some(vm),
            parent_containers: Vec::default(),
            debug: false,
        }
    }

    /// Makes a display context with the given VM and reserved capacity.
    pub fn with_vm_and_capacity(vm: &'a KotoVm, capacity: usize) -> Self {
        Self {
            result: String::with_capacity(capacity),
            vm: Some(vm),
            parent_containers: Vec::default(),
            debug: false,
        }
    }

    /// Enables the debug flag on the display context.
    pub fn enable_debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Returns the resulting string and consumes the context.
    pub fn result(self) -> String {
        self.result
    }

    /// Appends to the end of the string.
    pub fn append<'b>(&mut self, s: impl Into<StringBuilderAppend<'b>>) {
        s.into().append(&mut self.result);
    }

    /// Returns a reference to the context's VM.
    pub fn vm(&self) -> &Option<&'a KotoVm> {
        &self.vm
    }

    /// True if the resulting string will be used in a debug context.
    pub fn debug_enabled(&self) -> bool {
        self.debug
    }

    /// Returns true if the value that's being displayed is in a container.
    pub fn is_contained(&self) -> bool {
        !self.parent_containers.is_empty()
    }

    /// Returns true if the given ID is present in the parent container list.
    pub fn is_in_parents(&self, id: usize) -> bool {
        self.parent_containers.contains(&id)
    }

    /// Adds the given ID to the parents list.
    pub fn push_container(&mut self, id: usize) {
        self.parent_containers.push(id);
    }

    /// Pops the previously added parent ID.
    pub fn pop_container(&mut self) {
        self.parent_containers.pop();
    }
}

impl fmt::Write for DisplayContext<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.append(s);
        Ok(())
    }
}

/// Types that can be appended to [`DisplayContext`].
pub enum StringBuilderAppend<'a> {
    Char(char),
    Str(&'a str),
    String(String),
    KString(KString),
    KStringRef(&'a KString),
}

impl From<char> for StringBuilderAppend<'_> {
    fn from(value: char) -> Self {
        Self::Char(value)
    }
}

impl<'a> From<&'a str> for StringBuilderAppend<'a> {
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

impl From<String> for StringBuilderAppend<'_> {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<KString> for StringBuilderAppend<'_> {
    fn from(value: KString) -> Self {
        Self::KString(value)
    }
}

impl<'a> From<&'a KString> for StringBuilderAppend<'a> {
    fn from(value: &'a KString) -> Self {
        Self::KStringRef(value)
    }
}

impl StringBuilderAppend<'_> {
    fn append(self, string: &mut String) {
        match self {
            Self::Char(c) => string.push(c),
            Self::Str(s) => string.push_str(s),
            Self::String(s) => string.push_str(&s),
            Self::KString(s) => string.push_str(&s),
            Self::KStringRef(s) => string.push_str(s),
        }
    }
}
