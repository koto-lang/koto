use std::fmt;

use koto_memory::Address;

use crate::{KString, KotoVm};

/// A helper for converting Koto values to strings
#[derive(Default)]
pub struct DisplayContext<'a> {
    result: String,
    vm: Option<&'a KotoVm>,
    // A contained value might need to be displayed differently,
    // - Strings should be displayed with quotes when they're inside a container.
    // - Containers should check the parent list to avoid recursive display operations.
    parent_containers: Vec<Address>,
}

impl<'a> DisplayContext<'a> {
    /// Makes a display context with the given VM
    pub fn with_vm(vm: &'a KotoVm) -> Self {
        Self {
            result: String::default(),
            vm: Some(vm),
            parent_containers: Vec::default(),
        }
    }

    /// Makes a display context with the given VM and reserved capacity
    pub fn with_vm_and_capacity(vm: &'a KotoVm, capacity: usize) -> Self {
        Self {
            result: String::with_capacity(capacity),
            vm: Some(vm),
            parent_containers: Vec::default(),
        }
    }

    /// Appends to the end of the string
    pub fn append<'b>(&mut self, s: impl Into<StringBuilderAppend<'b>>) {
        s.into().append(&mut self.result);
    }

    /// Returns the resulting string and consumes the context
    pub fn result(self) -> String {
        self.result
    }

    /// Returns a reference to the context's VM
    pub fn vm(&self) -> &Option<&'a KotoVm> {
        &self.vm
    }

    /// Returns true if the value that's being displayed is in a container
    pub fn is_contained(&self) -> bool {
        !self.parent_containers.is_empty()
    }

    /// Returns true if the given ID is present in the parent container list
    pub fn is_in_parents(&self, id: Address) -> bool {
        self.parent_containers
            .iter()
            .any(|parent_id| *parent_id == id)
    }

    /// Adds the given ID to the parents list
    ///
    /// Containers should call this before displaying their contained values.
    pub fn push_container(&mut self, id: Address) {
        self.parent_containers.push(id);
    }

    /// Pops the previously added parent ID
    ///
    /// Containers should call this after displaying their contained values.
    pub fn pop_container(&mut self) {
        self.parent_containers.pop();
    }
}

impl<'a> fmt::Write for DisplayContext<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.append(s);
        Ok(())
    }
}

/// Types that can be appended to [DisplayContext]
pub enum StringBuilderAppend<'a> {
    Char(char),
    Str(&'a str),
    String(String),
    KString(KString),
    KStringRef(&'a KString),
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

impl From<KString> for StringBuilderAppend<'_> {
    fn from(value: KString) -> Self {
        StringBuilderAppend::KString(value)
    }
}

impl<'a> From<&'a KString> for StringBuilderAppend<'a> {
    fn from(value: &'a KString) -> Self {
        StringBuilderAppend::KStringRef(value)
    }
}

impl<'a> StringBuilderAppend<'a> {
    fn append(self, string: &mut String) {
        match self {
            StringBuilderAppend::Char(c) => string.push(c),
            StringBuilderAppend::Str(s) => string.push_str(s),
            StringBuilderAppend::String(s) => string.push_str(&s),
            StringBuilderAppend::KString(s) => string.push_str(&s),
            StringBuilderAppend::KStringRef(s) => string.push_str(s),
        }
    }
}
