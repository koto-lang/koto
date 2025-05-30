use std::{iter::Peekable, str::Chars};

use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{ConstantIndex, constant_pool::ConstantPoolBuilder};

/// The formatting options that are available for interpolated strings
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct StringFormatOptions {
    /// The alignment that padded strings should use
    pub alignment: StringAlignment,
    /// The minimum width that should be taken up by the string
    pub min_width: Option<u32>,
    /// The number of decimal places to use when formatting floats
    pub precision: Option<u32>,
    /// The character that padded strings should use to fill empty space
    pub fill_character: Option<ConstantIndex>,
    /// An alternative representation that can be applied to the formatted string
    pub representation: Option<StringFormatRepresentation>,
}

impl StringFormatOptions {
    /// Parses a format string
    pub(crate) fn parse(
        format_string: &str,
        constants: &mut ConstantPoolBuilder,
    ) -> Result<Self, StringFormatError> {
        use FormatParsePosition::*;
        let mut position = Start;
        let mut result = Self::default();
        let mut chars = format_string.chars().peekable();

        let char_to_alignment = |c: char| match c {
            '<' => StringAlignment::Left,
            '^' => StringAlignment::Center,
            '>' => StringAlignment::Right,
            _ => unreachable!(),
        };

        let mut add_string_constant = |s: &str| {
            constants
                .add_string(s)
                .map_err(|_| StringFormatError::InternalError)
        };

        while let Some(next) = chars.next() {
            match (next, chars.peek(), position) {
                // Check for single-char fill character at the start of the string
                (_, Some('<' | '^' | '>'), Start) => {
                    result.fill_character =
                        Some(add_string_constant(&format_string[0..next.len_utf8()])?);
                    result.alignment = char_to_alignment(chars.next().unwrap());
                    position = MinWidth;
                }
                ('<' | '^' | '>', _, Start | Alignment) => {
                    result.alignment = char_to_alignment(next);
                    position = MinWidth;
                }
                ('0', Some('0'..='9'), Start | MinWidth) => {
                    result.fill_character = Some(add_string_constant("0")?);
                    position = MinWidth;
                }
                ('0'..='9', _, Start | MinWidth) => {
                    result.min_width = Some(consume_u32(next, &mut chars)?);
                    position = Precision;
                }
                ('.', Some(_), Start | MinWidth | Precision) => {
                    let first_digit = chars.next().unwrap();
                    result.precision = Some(consume_u32(first_digit, &mut chars)?);
                    position = Type;
                }
                ('?', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::Debug);
                    position = End;
                }
                ('b', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::Binary);
                    position = End;
                }
                ('o', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::Octal);
                    position = End;
                }
                ('x', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::HexLower);
                    position = End;
                }
                ('X', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::HexUpper);
                    position = End;
                }
                ('e', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::ExpLower);
                    position = End;
                }
                ('E', _, Start | MinWidth | Precision | Type) => {
                    result.representation = Some(StringFormatRepresentation::ExpUpper);
                    position = End;
                }
                (_, _, Start) => {
                    // Unwrapping here is fine, format_string is valid UTF-8
                    let fill = format_string.graphemes(true).next().unwrap();
                    // The fill grapheme cluster can only appear at the start of the format string
                    chars = format_string[fill.len()..].chars().peekable();
                    result.fill_character = Some(add_string_constant(fill)?);
                    position = Alignment;
                }
                (other, _, _) => {
                    return Err(StringFormatError::UnexpectedToken(other));
                }
            }
        }

        Ok(result)
    }
}

// Used during parsing of a format string, see [StringFormatOptions::parse]
#[derive(Copy, Clone, Debug)]
enum FormatParsePosition {
    Start,
    Alignment,
    MinWidth,
    Precision,
    Type,
    End,
}

fn consume_u32(first: char, chars: &mut Peekable<Chars>) -> Result<u32, StringFormatError> {
    let mut n = first
        .to_digit(10)
        .ok_or(StringFormatError::ExpectedNumber(first))? as u64;
    let index_max = u32::MAX as u64;

    while let Some(n_next @ '0'..='9') = chars.peek().cloned() {
        chars.next();

        n *= 10;
        n += n_next
            .to_digit(10)
            .ok_or(StringFormatError::ExpectedNumber(first))? as u64;

        if n > index_max {
            return Err(StringFormatError::FormatNumberIsTooLarge(n));
        }
    }

    Ok(n as u32)
}

/// Alignment options for formatted strings
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum StringAlignment {
    /// Default alignment is right-aligned for numbers, left-aligned otherwise
    #[default]
    Default,
    Left,
    Center,
    Right,
}

/// Alternative representations formatted strings
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum StringFormatRepresentation {
    Debug,
    HexLower,
    HexUpper,
    Binary,
    Octal,
    ExpLower,
    ExpUpper,
}

impl TryFrom<u8> for StringFormatRepresentation {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        if byte == Self::Debug as u8 {
            Ok(Self::Debug)
        } else if byte == Self::HexLower as u8 {
            Ok(Self::HexLower)
        } else if byte == Self::HexUpper as u8 {
            Ok(Self::HexUpper)
        } else if byte == Self::Binary as u8 {
            Ok(Self::Binary)
        } else if byte == Self::Octal as u8 {
            Ok(Self::Octal)
        } else if byte == Self::ExpLower as u8 {
            Ok(Self::ExpLower)
        } else if byte == Self::ExpUpper as u8 {
            Ok(Self::ExpUpper)
        } else {
            Err(format!("Invalid string format style: {byte:#010b}"))
        }
    }
}

/// An error that represents a problem with the Parser's internal logic, rather than a user error
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum StringFormatError {
    #[error("expected a number '{0}'")]
    ExpectedNumber(char),
    #[error("{0} is larger than the maximum of {max}", max = u32::MAX)]
    FormatNumberIsTooLarge(u64),
    #[error("an unexpected internal error occurred")]
    InternalError,
    #[error("unexpected token '{0}'")]
    UnexpectedToken(char),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_format_string(cases: &[(&str, StringFormatOptions)]) {
        for (options, expected) in cases {
            let mut constants = ConstantPoolBuilder::default();
            assert_eq!(
                *expected,
                StringFormatOptions::parse(options, &mut constants).unwrap()
            );
        }
    }

    #[test]
    fn width_and_precision() {
        test_parse_format_string(&[
            (
                "10",
                StringFormatOptions {
                    min_width: Some(10),
                    ..Default::default()
                },
            ),
            (
                "08",
                StringFormatOptions {
                    fill_character: Some(0.into()),
                    min_width: Some(8),
                    ..Default::default()
                },
            ),
            (
                ".12",
                StringFormatOptions {
                    precision: Some(12),
                    ..Default::default()
                },
            ),
            (
                "5.9",
                StringFormatOptions {
                    min_width: Some(5),
                    precision: Some(9),
                    ..Default::default()
                },
            ),
        ])
    }

    #[test]
    fn fill_and_alignment() {
        test_parse_format_string(&[
            (
                "_^",
                StringFormatOptions {
                    alignment: StringAlignment::Center,
                    fill_character: Some(0.into()),
                    ..Default::default()
                },
            ),
            (
                "𝜇<.9",
                StringFormatOptions {
                    alignment: StringAlignment::Left,
                    fill_character: Some(0.into()),
                    precision: Some(9),
                    ..Default::default()
                },
            ),
            (
                "🫶🏽>20.10",
                StringFormatOptions {
                    alignment: StringAlignment::Right,
                    fill_character: Some(0.into()),
                    min_width: Some(20),
                    precision: Some(10),
                    ..Default::default()
                },
            ),
            (
                "<.8",
                StringFormatOptions {
                    alignment: StringAlignment::Left,
                    precision: Some(8),
                    ..Default::default()
                },
            ),
            (
                "}>2",
                StringFormatOptions {
                    alignment: StringAlignment::Right,
                    fill_character: Some(0.into()),
                    min_width: Some(2),
                    ..Default::default()
                },
            ),
            (
                "8^4",
                StringFormatOptions {
                    alignment: StringAlignment::Center,
                    fill_character: Some(0.into()),
                    min_width: Some(4),
                    ..Default::default()
                },
            ),
        ])
    }

    #[test]
    fn style() {
        test_parse_format_string(&[
            (
                "x",
                StringFormatOptions {
                    representation: Some(StringFormatRepresentation::HexLower),
                    ..Default::default()
                },
            ),
            (
                "X",
                StringFormatOptions {
                    representation: Some(StringFormatRepresentation::HexUpper),
                    ..Default::default()
                },
            ),
            (
                "o",
                StringFormatOptions {
                    representation: Some(StringFormatRepresentation::Octal),
                    ..Default::default()
                },
            ),
            (
                "b",
                StringFormatOptions {
                    representation: Some(StringFormatRepresentation::Binary),
                    ..Default::default()
                },
            ),
        ]);
    }
}
