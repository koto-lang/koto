use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub line: u32,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}
