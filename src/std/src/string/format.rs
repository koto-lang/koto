use {
    koto_lexer::{is_id_continue, is_id_start},
    koto_runtime::Value,
    std::sync::Arc,
};

#[derive(Debug, PartialEq)]
pub enum FormatToken<'a> {
    String(&'a str),
    Placeholder,
    Positional(u32),
    Identifier(&'a str),
    Error,
}

pub struct FormatLexer<'a> {
    format_string: &'a str,
    position: usize,
}

impl<'a> FormatLexer<'a> {
    pub fn new(format_string: &'a str) -> Self {
        Self {
            format_string,
            position: 0,
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

                        match chars.next() {
                            Some('{') => {
                                let result = &self.format_string[self.position..self.position + 1];
                                self.position += 1;
                                Some(String(result))
                            }
                            Some('}') => {
                                self.position += 1;
                                Some(Placeholder)
                            }
                            Some(n @ '0'..='9') => {
                                self.position += 1;
                                let mut n = n.to_digit(10).unwrap();

                                while let Some(c) = chars.next() {
                                    match c {
                                        n2 @ '0'..='9' => {
                                            self.position += 1;
                                            n *= 10;
                                            n += n2.to_digit(10).unwrap();
                                        }
                                        '}' => {
                                            self.position += 1;
                                            break;
                                        }
                                        _ => return Some(Error),
                                    }
                                }

                                Some(Positional(n))
                            }
                            Some(c) if is_id_start(c) => {
                                let start = self.position;
                                let mut end = start + 1;
                                self.position += 1;

                                while let Some(c) = chars.next() {
                                    match c {
                                        _ if is_id_continue(c) => {
                                            end += 1;
                                            self.position += 1;
                                        }
                                        '}' => {
                                            self.position += 1;
                                            break;
                                        }
                                        _ => return Some(Error),
                                    }
                                }

                                Some(Identifier(&self.format_string[start..end]))
                            }
                            _ => Some(Error),
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
                                        return Some(Error);
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

pub fn format_string(format_string: &str, format_args: &[Value]) -> Result<String, String> {
    let mut arg_iter = format_args.iter();
    let mut result = String::with_capacity(format_string.len());

    for token in FormatLexer::new(&format_string) {
        match token {
            FormatToken::String(s) => result.push_str(s),
            FormatToken::Placeholder => match arg_iter.next() {
                Some(arg) => result.push_str(&arg.to_string()),
                None => return Err("Not enough arguments for format string".to_string()),
            },
            FormatToken::Positional(n) => match format_args.get(n as usize) {
                Some(arg) => result.push_str(&arg.to_string()),
                None => return Err(format!("Missing argument for index {}", n)),
            },
            FormatToken::Identifier(id) => match format_args.first() {
                Some(Value::Map(map)) => {
                    // TODO pass in runtime's string cache
                    match map.data().get(&Value::Str(Arc::new(id.to_string()))) {
                        Some(value) => result.push_str(&value.to_string()),
                        None => return Err(format!("Key '{}' not found in map", id)),
                    }
                }
                Some(other) => {
                    return Err(format!("Expected map as first argument, found {}", other))
                }
                None => return Err(String::from("Expected map as first argument")),
            },
            FormatToken::Error => return Err("Error while parsing format string".to_string()),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        koto_runtime::{ValueHashMap, ValueMap},
    };

    mod lexer {
        use super::*;

        fn check_lexer_output(input: &str, tokens: &[FormatToken]) {
            let mut lexer = FormatLexer::new(input);

            for (i, token) in tokens.iter().enumerate() {
                match lexer.next() {
                    Some(output) => {
                        assert_eq!(&output, token, "mismatch at position {}", i);
                    }
                    None => {
                        panic!("Lexer stopped providing output at position {}", i);
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
                    FormatToken::Placeholder,
                    FormatToken::String("bar"),
                ],
            )
        }

        #[test]
        fn several_placeholders() {
            let input = "one{} two {} three{} four";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("one"),
                    FormatToken::Placeholder,
                    FormatToken::String(" two "),
                    FormatToken::Placeholder,
                    FormatToken::String(" three"),
                    FormatToken::Placeholder,
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
            let input = "foo {0}{1}{0} bar";

            check_lexer_output(
                input,
                &[
                    FormatToken::String("foo "),
                    FormatToken::Positional(0),
                    FormatToken::Positional(1),
                    FormatToken::Positional(0),
                    FormatToken::String(" bar"),
                ],
            )
        }

        #[test]
        fn identifier_placeholders() {
            let input = "x = {foo}";

            check_lexer_output(
                input,
                &[FormatToken::String("x = "), FormatToken::Identifier("foo")],
            )
        }
    }

    mod format_string {
        use super::*;

        fn check_format_output(format: &str, args: &[Value], expected: &str) {
            match format_string(format, args) {
                Ok(result) => assert_eq!(result, expected),
                Err(error) => panic!(error),
            }
        }

        #[test]
        fn positional_placeholders() {
            check_format_output("{} foo {0}", &[Value::Number(1.0)], "1 foo 1");
            check_format_output(
                "{1} - {0} {} - {}",
                &[Value::Number(2.0), Value::Empty],
                "() - 2 2 - ()",
            );
        }

        #[test]
        fn identifier_placeholders() {
            let mut map_data = ValueHashMap::new();
            map_data.insert("x".into(), Value::Number(42.0));
            map_data.insert("y".into(), Value::Number(-1.0));
            let map = Value::Map(ValueMap::with_data(map_data));

            check_format_output("{x} - {y}", &[map], "42 - -1");
        }
    }
}
