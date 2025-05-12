use crate::Result;
use koto_lexer::{Lexer, Token};
use koto_parser::{Span, SyntaxError};

/// Captures non-AST 'trivia' items that are needed for formatting, like comments and newlines
#[derive(Default)]
pub struct Trivia {
    // Captured trivia items
    items: Vec<TriviaItem>,
}

impl Trivia {
    pub fn parse(source: &str) -> Result<Self> {
        let mut items = Vec::default();

        // Used to keep track of the how many newlines in a row are found in the input
        let mut newline_count = 0;

        for token in Lexer::new(source) {
            // Reset the newline count if any token other than newlines or whitespace is encountered
            if !matches!(token.token, Token::NewLine | Token::Whitespace) {
                newline_count = 0;
            }

            let maybe_trivia = match token.token {
                Token::CommentSingle => Some(TriviaToken::CommentSingle),
                Token::CommentMulti => Some(TriviaToken::CommentMulti),
                Token::NewLine => {
                    newline_count += 1;
                    // Capture an `EmptyLine` item if 2 newlines after each other are encountered
                    if newline_count == 2 {
                        Some(TriviaToken::EmptyLine)
                    } else {
                        None
                    }
                }
                Token::Whitespace => None,
                Token::Error => {
                    return Err(koto_parser::Error::new(
                        SyntaxError::UnexpectedToken.into(),
                        token.span,
                    ));
                }
                // Other tokens can be skipped
                _ => None,
            };

            if let Some(trivia_token) = maybe_trivia {
                items.push(TriviaItem {
                    token: trivia_token,
                    span: token.span,
                });
            }
        }

        Ok(Self { items })
    }

    pub fn iter(&self) -> TriviaIterator {
        self.items.iter().peekable()
    }
}

pub type TriviaIterator<'a> = std::iter::Peekable<std::slice::Iter<'a, TriviaItem>>;

#[derive(Clone, Copy, Debug)]
pub struct TriviaItem {
    pub token: TriviaToken,
    pub span: Span,
}

/// Tokens that are captured as [Trivia] for formatting a Koto source file
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TriviaToken {
    /// Empty lines are captured once
    /// (i.e two empty lines in a row are capture as a single empty line)
    EmptyLine,
    /// A single-line comment
    CommentSingle,
    /// A multi-line comment
    CommentMulti,
}

#[cfg(test)]
mod tests {
    use super::*;
    use koto_parser::Position;

    fn check_trivia_items(source: &str, expected_items: &[(TriviaToken, Span)]) {
        let trivia = match Trivia::parse(source) {
            Ok(trivia) => trivia,
            Err(error) => panic!("failed to parse trivia: {error}"),
        };

        for (i, ((token, span), actual)) in expected_items.iter().zip(trivia.iter()).enumerate() {
            assert_eq!(*token, actual.token, "Item mismatch at position {i}");
            assert_eq!(*span, actual.span, "Span mismatch at position {i}");
        }

        assert_eq!(
            expected_items.len(),
            trivia.items.len(),
            "Item count mismatch"
        );
    }

    #[test]
    fn comments_and_empty_lines() {
        let source = "\
# Hello
x = 1 # abcdef

#-
Multiline comment
-#

x = #- Inline comment -# x + 1

return x
";

        use TriviaToken::*;
        check_trivia_items(
            source,
            &[
                (
                    CommentSingle,
                    Span {
                        start: Position { line: 0, column: 0 },
                        end: Position { line: 0, column: 7 },
                    },
                ),
                (
                    CommentSingle,
                    Span {
                        start: Position { line: 1, column: 6 },
                        end: Position {
                            line: 1,
                            column: 14,
                        },
                    },
                ),
                (
                    EmptyLine,
                    Span {
                        start: Position { line: 2, column: 0 },
                        end: Position { line: 3, column: 0 },
                    },
                ),
                (
                    CommentMulti,
                    Span {
                        start: Position { line: 3, column: 0 },
                        end: Position { line: 5, column: 2 },
                    },
                ),
                (
                    EmptyLine,
                    Span {
                        start: Position { line: 6, column: 0 },
                        end: Position { line: 7, column: 0 },
                    },
                ),
                (
                    CommentMulti,
                    Span {
                        start: Position { line: 7, column: 4 },
                        end: Position {
                            line: 7,
                            column: 24,
                        },
                    },
                ),
                (
                    EmptyLine,
                    Span {
                        start: Position { line: 8, column: 0 },
                        end: Position { line: 9, column: 0 },
                    },
                ),
            ],
        );
    }
}
