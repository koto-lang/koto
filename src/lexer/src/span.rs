use std::{fmt, ops::Range, str};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
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

// TODO remove once pest is removed from koto_parser
impl<'a> From<pest::Span<'a>> for Span {
    fn from(span: pest::Span) -> Self {
        let start_line_col = span.start_pos().line_col();
        let end_line_col = span.end_pos().line_col();
        Self {
            start: Position {
                line: start_line_col.0 as u32,
                column: start_line_col.1 as u32,
            },
            end: Position {
                line: end_line_col.0 as u32,
                column: end_line_col.1 as u32,
            },
        }
    }
}

fn get_slice<'a>(source: &'a str, line_start: usize, byte_span: &Range<usize>) -> &'a str {
    let slice_start = line_start;
    let slice_end = byte_span.end;

    // from_utf8_unchecked could be considered here if necessary
    str::from_utf8(&source.as_bytes()[slice_start..slice_end]).unwrap()
}

pub fn make_span(
    source: &str,
    line_number: usize,
    line_start: usize,
    byte_span: &Range<usize>,
) -> Span {
    let slice = get_slice(source, line_start, byte_span);

    let mut line = line_number;
    let mut column = 1;
    let mut result = Span::default();
    result.start.line = line_number as u32;

    for (byte_count, c) in slice.char_indices() {
        if line_start + byte_count == byte_span.start {
            result.start.column = column as u32;
        }
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    result.end.line = line as u32;
    result.end.column = column as u32;

    result
}

pub fn make_start_position(
    source: &str,
    line_number: usize,
    line_start: usize,
    byte_span: &Range<usize>,
) -> Position {
    let slice = get_slice(source, line_start, byte_span);

    let mut column = 1;

    for (byte_count, _) in slice.char_indices() {
        if line_start + byte_count == byte_span.start {
            return Position {
                line: line_number as u32,
                column: column as u32,
            };
        }
        column += 1;
    }

    unreachable!();
}

pub fn make_end_position(
    source: &str,
    line_number: usize,
    line_start: usize,
    byte_span: &Range<usize>,
) -> Position {
    let slice = get_slice(source, line_start, byte_span);

    let mut line = line_number;
    let mut column = 1;

    for c in slice.chars() {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    Position {
        line: line as u32,
        column: column as u32,
    }
}
