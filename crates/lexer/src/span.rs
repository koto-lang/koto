/// Represents a line/column position in a script
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    /// The position's line, counting from 0
    pub line: u32,
    /// The position's column, counting from 0
    pub column: u32,
}

/// A span is a range in the source code, represented by a start and end position
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// The span's start position
    pub start: Position,
    /// The span's end position
    pub end: Position,
}
