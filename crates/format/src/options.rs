/// Formatting options provided to [`format`]
#[derive(Clone, Copy)]
pub struct FormatOptions {
    /// The width in characters to use when inserting indents (default: 2)
    pub indent_width: u8,
    /// The maximum line length (default: 100)
    pub line_length: u8,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 2,
            line_length: 100,
        }
    }
}
