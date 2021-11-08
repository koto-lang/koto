use std::fmt;

/// Represents a line/column position in a script
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// The position's line, counting from 1
    pub line: u32,
    /// The position's column, counting from 1.
    pub column: u32,
}

impl Default for Position {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.line, self.column)
    }
}

/// A span is a range in the source code, represented by a start and end position
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Span {
    /// The span's start position
    pub start: Position,
    /// The span's end position
    pub end: Position,
}
