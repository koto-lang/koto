use logos::{Lexer, Logos};

#[derive(Clone, Copy)]
pub struct Extras {
    pub line_number: usize,
    pub line_start: usize,
    pub indent: usize,
}

impl Default for Extras {
    fn default() -> Self {
        Self {
            line_number: 1,
            line_start: 0,
            indent: 0,
        }
    }
}

fn next_line(lexer: &mut Lexer<Token>) {
    lexer.extras.line_number += 1;
    lexer.extras.line_start = lexer.span().start + 1; // +1 to skip \n
    lexer.extras.indent = 0;
}

fn next_line_indented(lexer: &mut Lexer<Token>) {
    lexer.extras.line_number += 1;
    lexer.extras.line_start = lexer.span().start + 1; // +1 to skip \n
    lexer.extras.indent = lexer.slice().len() - 1;
}

fn count_newlines(lexer: &mut Lexer<Token>) {
    lexer.extras.line_number += lexer.slice().matches("\n").count();
}

#[derive(Logos, Copy, Clone, Debug, PartialEq)]
#[logos(extras = Extras)]
pub enum Token {
    #[error]
    Error,

    #[regex(r"[ \t\f]+")]
    Whitespace,

    #[token("\n", next_line)]
    NewLine,

    #[regex(r"\n( )+", next_line_indented)]
    NewLineIndented,

    #[regex(r"#[^-].*")]
    CommentSingle,

    #[regex(r"#-([^-]|-[^#])*-#", count_newlines)]
    CommentMulti,

    #[regex(r"-?(0|[1-9][0-9]*)(\.[0-9]+)?(e(\+|\-)?[0-9]+)?")]
    Number,

    #[regex(r#""(?:[^"\\]|\\.)*""#)]
    Str,

    #[regex(r"[a-zA-Z][a-zA-Z0-9_]*")]
    Id,

    // Symbols
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("(")]
    ParenOpen,
    #[token(")")]
    ParenClose,
    #[token("|")]
    Function,
    #[token("[")]
    ListStart,
    #[token("]")]
    ListEnd,
    #[token("{")]
    MapStart,
    #[token("}")]
    MapEnd,
    #[token("_")]
    Placeholder,
    #[token("..")]
    Range,
    #[token("..=")]
    RangeInclusive,
    #[token(",")]
    Separator,

    // operators
    #[token("+")]
    Add,
    #[token("-")]
    Subtract,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("%")]
    Modulo,

    #[token("=")]
    Assign,
    #[token("+=")]
    AssignAdd,
    #[token("-=")]
    AssignSubtract,
    #[token("*=")]
    AssignMultiply,
    #[token("/=")]
    AssignDivide,
    #[token("%=")]
    AssignModulo,

    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,

    #[token(">")]
    Greater,
    #[token(">=")]
    GreaterOrEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessOrEqual,

    // Keywords
    #[token("and")]
    And,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("copy")]
    Copy,
    #[token("debug")]
    Debug,
    #[token("else")]
    Else,
    #[token("elseif")] // TODO investigate why 'else if' causes 'else' to fail matching
    ElseIf,
    #[token("export")]
    Export,
    #[token("false")]
    False,
    #[token("for")]
    For,
    #[token("if")]
    If,
    #[token("in")]
    In,
    #[token("not")]
    Not,
    #[token("num2")]
    Num2,
    #[token("num4")]
    Num4,
    #[token("or")]
    Or,
    #[token("return")]
    Return,
    #[token("then")]
    Then,
    #[token("true")]
    True,
    #[token("until")]
    Until,
    #[token("while")]
    While,
}

struct PeekedToken<'a> {
    token: Option<Token>,
    span: logos::Span,
    slice: &'a str,
    extras: Extras,
}

pub struct KotoLexer<'a> {
    lexer: Lexer<'a, Token>,
    peeked_tokens: Vec<PeekedToken<'a>>,
    current_peek_index: usize,
}

impl<'a> KotoLexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            lexer: Token::lexer(source),
            peeked_tokens: Vec::new(),
            current_peek_index: 0,
        }
    }

    pub fn peek(&mut self) -> Option<Token> {
        if self.peeked_tokens.is_empty() {
            self.peek_n(0)
        } else {
            self.peeked_tokens[self.current_peek_index].token
        }
    }

    pub fn peek_n(&mut self, n: usize) -> Option<Token> {
        while self.peeked_tokens.len() - self.current_peek_index <= n {
            let span = self.lexer.span();
            let slice = self.lexer.slice();
            let extras = self.lexer.extras;
            // getting the token needs to happen after the other properties
            let token = self.lexer.next();
            self.peeked_tokens.push(PeekedToken {
                token,
                span,
                slice,
                extras,
            });
        }
        self.peeked_tokens[self.current_peek_index + n].token
    }

    pub fn source(&self) -> &'a str {
        self.lexer.source()
    }

    pub fn span(&self) -> logos::Span {
        if self.peeked_tokens.is_empty() {
            self.lexer.span()
        } else {
            self.peeked_tokens[self.current_peek_index].span.clone()
        }
    }

    pub fn slice(&self) -> &'a str {
        if self.peeked_tokens.is_empty() {
            self.lexer.slice()
        } else {
            self.peeked_tokens[self.current_peek_index].slice
        }
    }

    pub fn extras(&self) -> Extras {
        if self.peeked_tokens.is_empty() {
            self.lexer.extras
        } else {
            self.peeked_tokens[self.current_peek_index].extras
        }
    }

    pub fn current_indent(&self) -> usize {
        if self.peeked_tokens.is_empty() {
            self.lexer.extras.indent
        } else {
            self.peeked_tokens[self.current_peek_index].extras.indent
        }
    }

    pub fn next_indent(&self) -> usize {
        if self.current_peek_index < self.peeked_tokens.len() - 1 {
            self.peeked_tokens[self.current_peek_index + 1]
                .extras
                .indent
        } else {
            self.lexer.extras.indent
        }
    }

    pub fn next_span(&self) -> logos::Span {
        self.lexer.span()
    }
}

impl<'a> Iterator for KotoLexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        if self.peeked_tokens.is_empty() {
            self.lexer.next()
        } else {
            let result = self.peeked_tokens[self.current_peek_index].token;
            self.current_peek_index += 1;
            if self.current_peek_index == self.peeked_tokens.len() {
                self.peeked_tokens.clear();
                self.current_peek_index = 0;
            }
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Token::*, *};

    fn check_lexer_output(source: &str, tokens: &[(Token, Option<&str>, u32)]) {
        let mut lex = Token::lexer(source);

        for (token, maybe_slice, line_number) in tokens {
            loop {
                match lex.next().expect("Expected token") {
                    Whitespace => continue,
                    output => {
                        assert_eq!(&output, token);
                        if let Some(slice) = maybe_slice {
                            assert_eq!(&lex.slice(), slice);
                        }
                        assert_eq!(lex.extras.line_number as u32, *line_number);
                        break;
                    }
                }
            }
        }

        assert_eq!(lex.next(), None);
    }

    fn check_lexer_output_indented(source: &str, tokens: &[(Token, Option<&str>, u32, u32)]) {
        let mut lex = Token::lexer(source);

        for (i, (token, maybe_slice, line_number, indent)) in tokens.iter().enumerate() {
            loop {
                match lex.next().expect("Expected token") {
                    Whitespace => continue,
                    output => {
                        assert_eq!(&output, token, "token {}", i);
                        if let Some(slice) = maybe_slice {
                            assert_eq!(&lex.slice(), slice, "token {}", i);
                        }
                        assert_eq!(
                            lex.extras.line_number as u32, *line_number,
                            "Line number (token {})",
                            i
                        );
                        assert_eq!(lex.extras.indent as u32, *indent, "Indent (token {})", i);
                        break;
                    }
                }
            }
        }

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn ids() {
        let input = "id id1 id_2 i_d_3 if _";
        check_lexer_output(
            input,
            &[
                (Id, Some("id"), 1),
                (Id, Some("id1"), 1),
                (Id, Some("id_2"), 1),
                (Id, Some("i_d_3"), 1),
                (If, None, 1),
                (Placeholder, None, 1),
            ],
        );
    }

    #[test]
    fn indent() {
        // TODO add indentation to test
        let input = "\
if true then

num4 1
num2 2
x
y";
        check_lexer_output(
            input,
            &[
                (If, None, 1),
                (True, None, 1),
                (Then, None, 1),
                (NewLine, None, 2),
                (NewLine, None, 3),
                (Num4, None, 3),
                (Number, Some("1"), 3),
                (NewLine, None, 4),
                (Num2, None, 4),
                (Number, Some("2"), 4),
                (NewLine, None, 5),
                (Id, Some("x"), 5),
                (NewLine, None, 6),
                (Id, Some("y"), 6),
            ],
        );
    }

    #[test]
    fn comments() {
        let input = "\
# single
true #-
multiline -
false #
-# true
()";
        check_lexer_output(
            input,
            &[
                (CommentSingle, Some("# single"), 1),
                (NewLine, None, 2),
                (True, None, 2),
                (CommentMulti, Some("#-\nmultiline -\nfalse #\n-#"), 5),
                (True, None, 5),
                (NewLine, None, 6),
                (ParenOpen, None, 6),
                (ParenClose, None, 6),
            ],
        );
    }

    #[test]
    fn strings() {
        let input = r#"
"hello, world!"
"escaped \"\n string"
true"#;
        check_lexer_output(
            input,
            &[
                (NewLine, None, 2),
                (Str, Some(r#""hello, world!""#), 2),
                (NewLine, None, 3),
                (Str, Some(r#""escaped \"\n string""#), 3),
                (NewLine, None, 4),
                (True, None, 4),
            ],
        );
    }

    #[test]
    fn numbers() {
        let input = "\
123
55.5
-1e-3
0.5e+9
-8e8";
        check_lexer_output(
            input,
            &[
                (Number, Some("123"), 1),
                (NewLine, None, 2),
                (Number, Some("55.5"), 2),
                (NewLine, None, 3),
                (Number, Some("-1e-3"), 3),
                (NewLine, None, 4),
                (Number, Some("0.5e+9"), 4),
                (NewLine, None, 5),
                (Number, Some("-8e8"), 5),
            ],
        );
    }

    #[test]
    fn ranges() {
        let input = "\
a[..=9]
x = [i for i in 0..5]";
        check_lexer_output(
            input,
            &[
                (Id, Some("a"), 1),
                (ListStart, None, 1),
                (RangeInclusive, None, 1),
                (Number, Some("9"), 1),
                (ListEnd, None, 1),
                (NewLine, None, 2),
                (Id, Some("x"), 2),
                (Assign, None, 2),
                (ListStart, None, 2),
                (Id, Some("i"), 2),
                (For, None, 2),
                (Id, Some("i"), 2),
                (In, None, 2),
                (Number, Some("0"), 2),
                (Range, None, 2),
                (Number, Some("5"), 2),
                (ListEnd, None, 2),
            ],
        );
    }

    #[test]
    fn function() {
        let input = "\
export f = |a b|
  c = a + b
  c
f()";
        check_lexer_output_indented(
            input,
            &[
                (Export, None, 1, 0),
                (Id, Some("f"), 1, 0),
                (Assign, None, 1, 0),
                (Function, None, 1, 0),
                (Id, Some("a"), 1, 0),
                (Id, Some("b"), 1, 0),
                (Function, None, 1, 0),
                (NewLineIndented, None, 2, 2),
                (Id, Some("c"), 2, 2),
                (Assign, None, 2, 2),
                (Id, Some("a"), 2, 2),
                (Add, None, 2, 2),
                (Id, Some("b"), 2, 2),
                (NewLineIndented, None, 3, 2),
                (Id, Some("c"), 3, 2),
                (NewLine, None, 4, 0),
                (Id, Some("f"), 4, 0),
                (ParenOpen, None, 4, 0),
                (ParenClose, None, 4, 0),
            ],
        );
    }

    #[test]
    fn if_inline() {
        let input = "1 + if true then 0 else 1";
        check_lexer_output(
            input,
            &[
                (Number, Some("1"), 1),
                (Add, None, 1),
                (If, None, 1),
                (True, None, 1),
                (Then, None, 1),
                (Number, Some("0"), 1),
                (Else, None, 1),
                (Number, Some("1"), 1),
            ],
        );
    }

    #[test]
    fn if_block() {
        let input = "\
if true
  0
elseif false
  1
else
  0";
        check_lexer_output_indented(
            input,
            &[
                (If, None, 1, 0),
                (True, None, 1, 0),
                (NewLineIndented, None, 2, 2),
                (Number, Some("0"), 2, 2),
                (NewLine, None, 3, 0),
                (ElseIf, None, 3, 0),
                (False, None, 3, 0),
                (NewLineIndented, None, 4, 2),
                (Number, Some("1"), 4, 2),
                (NewLine, None, 5, 0),
                (Else, None, 5, 0),
                (NewLineIndented, None, 6, 2),
                (Number, Some("0"), 6, 2),
            ],
        );
    }
}
