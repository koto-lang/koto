/// Represents a line/column position in a script
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    /// The position's line, counting from 0
    pub line: u32,
    /// The position's column, counting from 0
    pub column: u32,
}

/// A span is a range in the source code, represented by a start and end position
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    /// The span's start position
    pub start: Position,
    /// The span's end position
    pub end: Position,
}

impl Span {
    /// Returns a span with zero size at the start of the given line
    pub fn line_start(line: u32) -> Self {
        let position = Position { line, column: 0 };
        Self {
            start: position,
            end: position,
        }
    }
}
