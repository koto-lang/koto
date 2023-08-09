//! String formatting support for `string.format` and `io.print`

use unicode_segmentation::UnicodeSegmentation;

use crate::{runtime_error, RuntimeError, UnaryOp, Value, Vm};
use koto_lexer::{is_id_continue, is_id_start};
use std::{iter::Peekable, str::Chars};

#[derive(Debug, PartialEq, Eq)]
enum FormatToken<'a> {
    String(&'a str),
    Placeholder(FormatSpec),
    Positional(u32, FormatSpec),
    Identifier(&'a str, FormatSpec),
    Error(String),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct FormatSpec {
    fill: Option<char>,
    alignment: Option<FormatAlign>,
    min_width: Option<u32>,
    precision: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
enum FormatAlign {
    Left,
    Center,
    Right,
}

struct FormatLexer<'a> {
    format_string: &'a str,
    position: usize,
}

impl<'a> FormatLexer<'a> {
    fn new(format_string: &'a str) -> Self {
        Self {
            format_string,
            position: 0,
        }
    }

    fn consume_format_spec(&mut self, chars: &mut Peekable<Chars>) -> Result<FormatSpec, String> {
        let mut result = FormatSpec::default();

        if let Some(maybe_fill) = chars.peek().cloned() {
            let mut lookahead = chars.clone();
            lookahead.next();
            if matches!(lookahead.next(), Some('<' | '^' | '>')) {
                chars.next();
                self.position += maybe_fill.len_utf8();
                result.fill = Some(maybe_fill);
            }
        }

        match chars.peek() {
            Some('<') => {
                chars.next();
                self.position += 1;
                result.alignment = Some(FormatAlign::Left);
            }
            Some('^') => {
                chars.next();
                self.position += 1;
                result.alignment = Some(FormatAlign::Center);
            }
            Some('>') => {
                chars.next();
                self.position += 1;
                result.alignment = Some(FormatAlign::Right);
            }
            _ => {}
        }

        if matches!(chars.peek(), Some('0'..='9')) {
            result.min_width = Some(self.consume_u32(chars)?);
        }

        if matches!(chars.peek(), Some('.')) {
            chars.next();
            self.position += 1;
            result.precision = Some(self.consume_u32(chars)?);
        }

        match chars.peek() {
            Some('}') => {
                chars.next();
                self.position += 1;
                Ok(result)
            }
            Some(other) => Err(format!("Expected '}}', found '{other}'")),
            None => Err("Expected '}}'".to_string()),
        }
    }

    fn consume_u32(&mut self, chars: &mut Peekable<Chars>) -> Result<u32, String> {
        match chars.next() {
            Some(n @ '0'..='9') => {
                self.position += 1;
                let mut n = n.to_digit(10).unwrap() as u64;
                let index_max = u32::MAX as u64;

                while let Some(n_next @ '0'..='9') = chars.peek().cloned() {
                    chars.next();
                    self.position += 1;

                    n *= 10;
                    n += n_next.to_digit(10).unwrap() as u64;

                    if n > index_max {
                        return Err(format!(
                            "Placeholder index exceeds the maximum of {index_max}"
                        ));
                    }
                }

                Ok(n as u32)
            }
            Some(other) => Err(format!("Expected digit, found '{other}'")),
            None => Err("Expected digit".into()),
        }
    }
}

impl<'a> Iterator for FormatLexer<'a> {
    type Item = FormatToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        use FormatToken::*;

        match self.format_string.get(self.position..) {
            Some(remaining) => {
                let mut chars = remaining.chars().peekable();

                match chars.peek() {
                    Some('{') => {
                        chars.next();
                        self.position += 1;

                        match chars.peek() {
                            // An escaped '{', e.g. "{{"
                            // This will only match at the start of a format string,
                            // further escaped '{'s will be matched by the Some(_) handler below.
                            Some('{') => {
                                let result = &self.format_string[self.position..self.position + 1];
                                chars.next();
                                self.position += 1;
                                Some(String(result))
                            }
                            // An empty placeholder, e.g. "{}"
                            Some('}') => {
                                chars.next();
                                self.position += 1;
                                Some(Placeholder(FormatSpec::default()))
                            }
                            // A positional placeholder, e.g. "{0}", "{9}", etc.
                            Some('0'..='9') => {
                                let mut format_spec = FormatSpec::default();

                                let placeholder_index = match self.consume_u32(&mut chars) {
                                    Ok(index) => match chars.next() {
                                        Some('}') => {
                                            self.position += 1;
                                            index
                                        }
                                        Some(':') => {
                                            self.position += 1;
                                            match self.consume_format_spec(&mut chars) {
                                                Ok(spec) => {
                                                    format_spec = spec;
                                                    index
                                                }
                                                Err(error) => return Some(Error(error)),
                                            }
                                        }
                                        Some(other) => {
                                            return Some(Error(format!(
                                                "Unexpected character '{other}'",
                                            )))
                                        }
                                        None => {
                                            return Some(Error(
                                                "Unexpected end of format argument".to_string(),
                                            ))
                                        }
                                    },
                                    Err(error) => return Some(Error(error)),
                                };

                                Some(Positional(placeholder_index, format_spec))
                            }
                            // An ID placeholder, e.g. "{x}", "{y}", etc.
                            Some(c) if is_id_start(*c) => {
                                let mut format_spec = FormatSpec::default();

                                let start = self.position;
                                let mut end = start + 1;
                                chars.next();
                                self.position += 1;

                                while let Some(c) = chars.next() {
                                    self.position += 1;
                                    match c {
                                        _ if is_id_continue(c) => {
                                            end += 1;
                                        }
                                        ':' => match self.consume_format_spec(&mut chars) {
                                            Ok(spec) => {
                                                format_spec = spec;
                                                break;
                                            }
                                            Err(error) => return Some(Error(error)),
                                        },
                                        '}' => {
                                            break;
                                        }
                                        other => {
                                            return Some(Error(format!(
                                                "Unexpected character '{other}'",
                                            )))
                                        }
                                    }
                                }

                                Some(Identifier(&self.format_string[start..end], format_spec))
                            }
                            // The start of a formatting specifier, e.g. "{:.2}"
                            Some(':') => {
                                chars.next();
                                self.position += 1;

                                match self.consume_format_spec(&mut chars) {
                                    Ok(spec) => Some(Placeholder(spec)),
                                    Err(error) => Some(Error(error)),
                                }
                            }
                            Some(other) => Some(Error(format!("Unexpected character - '{other}'"))),
                            None => Some(Error("Unexpected end, missing '}}'".into())),
                        }
                    }
                    Some(_) => {
                        let start = self.position;
                        let mut end = self.position;

                        while let Some(c) = chars.next() {
                            match c {
                                '{' => {
                                    if chars.next() == Some('{') {
                                        // A double open-brace ends the literal,
                                        // but includes only one of open braces,
                                        // which is why we advance the position by 2.
                                        end += 1;
                                        self.position += 2;
                                    } else {
                                        // A single open-brace is the start of a placeholder,
                                        // so don't advance the position, the brace will be
                                        // consumed in the next iteration.
                                    }
                                    break;
                                }
                                '}' => {
                                    if chars.next() == Some('}') {
                                        // A double close-brace ends the literal,
                                        // but includes only one of close braces,
                                        // which is why we advance the position by 2.
                                        end += 1;
                                        self.position += 2;
                                    } else {
                                        // An unescaped close-brace shouldn't be encountered
                                        // outside of a placeholder
                                        return Some(Error(
                                            "Encountered an unescaped '}}'".to_string(),
                                        ));
                                    }
                                    break;
                                }
                                _ => {
                                    end += 1;
                                    self.position += 1;
                                }
                            }
                        }

                        Some(String(&self.format_string[start..end]))
                    }
                    None => None,
                }
            }
            None => None,
        }
    }
}

/// Formats a string, used by `string.format` and `io.print`
pub fn format_string(
    vm: &mut Vm,
    format_string: &str,
    format_args: &[Value],
) -> Result<String, RuntimeError> {
    let mut arg_iter = format_args.iter();
    let mut result = String::with_capacity(format_string.len());

    for token in FormatLexer::new(format_string) {
        match token {
            FormatToken::String(s) => result.push_str(s),
            FormatToken::Placeholder(format_spec) => match arg_iter.next() {
                Some(arg) => result.push_str(&value_to_string(vm, arg, format_spec)?),
                None => return runtime_error!("Not enough arguments for format string"),
            },
            FormatToken::Positional(n, format_spec) => match format_args.get(n as usize) {
                Some(arg) => result.push_str(&value_to_string(vm, arg, format_spec)?),
                None => return runtime_error!("Missing argument for index {n}"),
            },
            FormatToken::Identifier(id, format_spec) => match format_args.first() {
                Some(Value::Map(map)) => match map.data().get(id) {
                    Some(value) => result.push_str(&value_to_string(vm, value, format_spec)?),
                    None => return runtime_error!("Key '{id}' not found in map"),
                },
                Some(other) => {
                    return runtime_error!(
                        "Expected map as first argument, found '{}'",
                        other.type_as_string()
                    )
                }
                None => return runtime_error!("Expected map as first argument"),
            },
            FormatToken::Error(error) => return runtime_error!("Invalid format string: {error}"),
        }
    }

    Ok(result)
}

fn value_to_string(
    vm: &mut Vm,
    value: &Value,
    format_spec: FormatSpec,
) -> Result<String, RuntimeError> {
    let result = match value {
        Value::Number(n) => match format_spec.precision {
            Some(precision) => {
                if n.is_f64() || n.is_i64_in_f64_range() {
                    format!("{:.*}", precision as usize, f64::from(n))
                } else {
                    n.to_string()
                }
            }
            None => n.to_string(),
        },
        _ => match vm.run_unary_op(UnaryOp::Display, value.clone())? {
            Value::Str(result) => {
                match format_spec.precision {
                    Some(precision) => {
                        // precision acts as a maximum width for non-number values
                        let mut truncated =
                            String::with_capacity((precision as usize).min(result.len()));
                        for grapheme in result.graphemes(true).take(precision as usize) {
                            truncated.push_str(grapheme);
                        }
                        truncated
                    }
                    None => result.to_string(),
                }
            }
            other => {
                return runtime_error!(
                    "Expected string from @display, found '{}'",
                    other.type_as_string()
                )
            }
        },
    };

    let result = match format_spec.min_width {
        Some(min_width) => {
            let min_width = min_width as usize;
            let len = result.graphemes(true).count();
            if len < min_width {
                let fill = format_spec.fill.unwrap_or(' ').to_string();
                let fill_chars = min_width - len;

                match format_spec.alignment {
                    Some(FormatAlign::Left) => result + &fill.repeat(fill_chars),
                    Some(FormatAlign::Center) => {
                        let half_fill_chars = fill_chars as f32 / 2.0;
                        format!(
                            "{}{}{}",
                            fill.repeat(half_fill_chars.floor() as usize),
                            result,
                            fill.repeat(half_fill_chars.ceil() as usize),
                        )
                    }
                    Some(FormatAlign::Right) => fill.repeat(fill_chars) + &result,
                    None => {
                        if matches!(value, Value::Number(_)) {
                            fill.repeat(fill_chars) + &result
                        } else {
                            result + &fill.repeat(fill_chars)
                        }
                    }
                }
            } else {
                result
            }
        }
        None => result,
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataMap, ValueMap};

    fn spec_with_precision(precision: u32) -> FormatSpec {
        FormatSpec {
            precision: Some(precision),
            ..Default::default()
        }
    }

    mod lexer {
        use super::*;

        fn check_lexer_output(input: &str, tokens: &[FormatToken]) {
            let mut lexer = FormatLexer::new(input);

            for (i, token) in tokens.iter().enumerate() {
                match lexer.next() {
                    Some(output) => {
                        assert_eq!(
                            &output, token,
                            "mismatch at position {i}, expected: {token:?}, actual: {output:?}",
                        );
                    }
                    None => {
                        panic!("Lexer stopped providing output at position {i}");
                    }
                }
            }

            assert_eq!(lexer.next(), None, "Lexer still has remaining output");
        }

        #[test]
        fn single_string() {
            let input = "hello";

            check_lexer_output(input, &[FormatToken::String("hello")])
        }

        #[test]
        fn single_placeholder() {
            let input = "foo{}bar";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("foo"),
                    FormatToken::Placeholder(FormatSpec::default()),
                    FormatToken::String("bar"),
                ],
            )
        }

        #[test]
        fn several_placeholders() {
            let input = "one{} two {} three{:.3} four";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("one"),
                    FormatToken::Placeholder(FormatSpec::default()),
                    FormatToken::String(" two "),
                    FormatToken::Placeholder(FormatSpec::default()),
                    FormatToken::String(" three"),
                    FormatToken::Placeholder(spec_with_precision(3)),
                    FormatToken::String(" four"),
                ],
            )
        }

        #[test]
        fn escaped_placeholder() {
            let input = "{{foo{{}}bar}}";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("{"),
                    FormatToken::String("foo{"),
                    FormatToken::String("}"),
                    FormatToken::String("bar}"),
                ],
            )
        }

        #[test]
        fn positional_placeholders() {
            let input = "foo {0}{1:.2}{0:_>5.3} bar";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("foo "),
                    FormatToken::Positional(0, FormatSpec::default()),
                    FormatToken::Positional(1, spec_with_precision(2)),
                    FormatToken::Positional(
                        0,
                        FormatSpec {
                            fill: Some('_'),
                            alignment: Some(FormatAlign::Right),
                            min_width: Some(5),
                            precision: Some(3),
                        },
                    ),
                    FormatToken::String(" bar"),
                ],
            )
        }

        #[test]
        fn identifier_placeholders() {
            let input = "x = {foo}, y = {bar:.2}";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("x = "),
                    FormatToken::Identifier("foo", FormatSpec::default()),
                    FormatToken::String(", y = "),
                    FormatToken::Identifier("bar", spec_with_precision(2)),
                ],
            )
        }
    }

    mod format_string {
        use super::*;

        fn check_format_output(format: &str, args: &[Value], expected: &str) {
            let mut vm = Vm::default();
            match format_string(&mut vm, format, args) {
                Ok(result) => assert_eq!(result, expected),
                Err(error) => panic!("format_string failed: '{error}'"),
            }
        }

        #[test]
        fn positional_placeholders() {
            check_format_output("{} foo {0}", &[Value::Number(1.into())], "1 foo 1");
            check_format_output(
                "{1} - {0} {} - {}",
                &[Value::Number(2.into()), Value::Null],
                "null - 2 2 - null",
            );
        }

        #[test]
        fn positional_with_precision() {
            let one = Value::Number(1.into());
            let one_third = Value::Number((1.0 / 3.0).into());
            check_format_output("{:.0}", &[one.clone()], "1");
            check_format_output("{:.2}", &[one.clone()], "1.00");
            check_format_output("{:.2}", &[one_third.clone()], "0.33");
            check_format_output("{:.3}", &[Value::Str("abcdef".into())], "abc");
            check_format_output("{0:.1}, {1:.3}", &[one, one_third], "1.0, 0.333");
        }

        #[test]
        fn identifier_placeholders() {
            let mut map_data = DataMap::default();
            map_data.insert("x".into(), Value::Number(42.into()));
            map_data.insert("y".into(), Value::Number(i64::from(-1).into()));
            let map = Value::Map(ValueMap::with_data(map_data));

            check_format_output("{x} - {y}", &[map.clone()], "42 - -1");
            check_format_output("{x:.2} - {y:.1}", &[map], "42.00 - -1.0");
        }

        #[test]
        fn fill_and_align_string() {
            let s = &[Value::Str("abcd".into())];
            check_format_output("{:8}", s, "abcd    ");
            check_format_output("{:<8}", s, "abcd    ");
            check_format_output("{:^8}", s, "  abcd  ");
            check_format_output("{:>8}", s, "    abcd");

            check_format_output("{:<8.2}", s, "ab      ");
            check_format_output("{:^8.2}", s, "   ab   ");
            check_format_output("{:>8.2}", s, "      ab");

            check_format_output("{:^8.3}", s, "  abc   ");

            check_format_output("{:_<8}", s, "abcd____");
            check_format_output("{:^^8}", s, "^^abcd^^");
            check_format_output("{:ß>8}", s, "ßßßßabcd");

            check_format_output("{:2}", s, "abcd");
        }

        #[test]
        fn fill_and_align_number() {
            let n = &[Value::Number((1.0 / 3.0).into())];
            let n_negative = &[Value::Number((-1.0 / 3.0).into())];
            check_format_output("{:8.2}", n, "    0.33");
            check_format_output("{:8.3}", n, "   0.333");
            check_format_output("{:®^8.3}", n, "®0.333®®");

            check_format_output("{:8.2}", n_negative, "   -0.33");
            check_format_output("{:^8.2}", n_negative, " -0.33  ");
            check_format_output("{:-<8.2}", n_negative, "-0.33---");
            check_format_output("{:8.3}", n_negative, "  -0.333");
        }
    }
}
