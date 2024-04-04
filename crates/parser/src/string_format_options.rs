use std::{iter::Peekable, str::Chars};

use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{constant_pool::ConstantPoolBuilder, ConstantIndex};

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
}

impl StringFormatOptions {
    /// TODO
    #[allow(unused)]
    pub(crate) fn parse(
        format_string: &str,
        constants: &mut ConstantPoolBuilder,
    ) -> Result<Self, StringFormatError> {
        use FormatParsePosition::*;
        let mut position = Start;
        let mut result = Self::default();
        let mut chars = format_string.chars().peekable();

        while let Some(next) = chars.peek() {
            match (next, position) {
                ('0'..='9', Start | MinWidth) => {
                    result.min_width = Some(consume_u32(&mut chars)?);
                    position = Precision;
                }
                ('.', Start | MinWidth | Precision) => {
                    chars.next();
                    result.precision = Some(consume_u32(&mut chars)?);
                    position = End;
                }
                ('<' | '^' | '>', Start | Alignment) => {
                    result.alignment = match chars.next().unwrap() {
                        '<' => StringAlignment::Left,
                        '^' => StringAlignment::Center,
                        '>' => StringAlignment::Right,
                        _ => unreachable!(),
                    };
                    position = MinWidth;
                }
                (_, Start) => {
                    // Unwrapping here is fine, format_string is valid UTF-8
                    let fill = format_string.graphemes(true).next().unwrap();
                    // The fill grapheme cluster can only appear at the start of the format string
                    chars = format_string[fill.len()..].chars().peekable();
                    let fill_constant = constants
                        .add_string(fill)
                        .map_err(|_| StringFormatError::InternalError)?;
                    result.fill_character = Some(fill_constant);
                    position = Alignment;
                }
                (other, _) => {
                    return Err(StringFormatError::UnexpectedToken(*other));
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
    End,
}

fn consume_u32(chars: &mut Peekable<Chars>) -> Result<u32, StringFormatError> {
    match chars.next() {
        Some(n @ '0'..='9') => {
            let mut n = n.to_digit(10).unwrap() as u64;
            let index_max = u32::MAX as u64;

            while let Some(n_next @ '0'..='9') = chars.peek().cloned() {
                chars.next();

                n *= 10;
                n += n_next.to_digit(10).unwrap() as u64;

                if n > index_max {
                    return Err(StringFormatError::FormatNumberIsTooLarge(n));
                }
            }

            Ok(n as u32)
        }
        _ => Err(StringFormatError::ExpectedNumber),
    }
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

/// An error that represents a problem with the Parser's internal logic, rather than a user error
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
pub enum StringFormatError {
    #[error("An unexpected internal error occurred")]
    InternalError,
    #[error("Expected a number")]
    ExpectedNumber,
    #[error("Unexpected token '{0}'")]
    UnexpectedToken(char),
    #[error("{0} is larger than the maximum of {}", u32::MAX)]
    FormatNumberIsTooLarge(u64),
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
                    fill_character: Some(0),
                    ..Default::default()
                },
            ),
            (
                "🫶🏽>20.10",
                StringFormatOptions {
                    alignment: StringAlignment::Right,
                    fill_character: Some(0),
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
                    fill_character: Some(0),
                    min_width: Some(2),
                    ..Default::default()
                },
            ),
        ])
    }
}
