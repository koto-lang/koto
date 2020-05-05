use logos::{Lexer, Logos};

// fn trim_str(start: usize, end: usize, s: &str) -> &str {
//     let len = s.len();
//     &s[start..len - end]
// }

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
    lexer.extras.line_start = lexer.span().start + 2; // +2 to skip \n
    lexer.extras.indent = 0;
}

fn next_line_indented(lexer: &mut Lexer<Token>) {
    lexer.extras.line_number += 1;
    lexer.extras.line_start = lexer.span().start + 2; // +2 to skip \n
    lexer.extras.indent = lexer.slice().len() - 1;
}

fn count_newlines(lexer: &mut Lexer<Token>) {
    lexer.extras.line_number += lexer.slice().matches("\n").count();
}

#[derive(Logos, Debug, PartialEq)]
#[logos(extras = Extras)]
pub enum Token {
    #[regex(r"[ \t\f]+", logos::skip)]
    #[error]
    Error,

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
    String,

    #[regex(r"[a-zA-Z][a-zA-Z0-9_]*")]
    Id,

    // Symbols
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("(")]
    ExpressionStart,
    #[token(")")]
    ExpressionEnd,
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

#[cfg(test)]
mod tests {
    use super::{Token::*, *};

    fn check_lexer_output(source: &str, tokens: &[(Token, Option<&str>, usize)]) {
        let mut lex = Token::lexer(source);

        for (token, maybe_slice, line_number) in tokens {
            assert_eq!(&lex.next().expect("Expected token"), token);
            if let Some(slice) = maybe_slice {
                assert_eq!(&lex.slice(), slice);
            }
            assert_eq!(&lex.extras.line_number, line_number);
        }

        assert_eq!(lex.next(), None);
    }

    fn check_lexer_output_indented(source: &str, tokens: &[(Token, Option<&str>, usize, usize)]) {
        let mut lex = Token::lexer(source);

        for (token, maybe_slice, line_number, indent) in tokens {
            assert_eq!(&lex.next().expect("Expected token"), token);
            if let Some(slice) = maybe_slice {
                assert_eq!(&lex.slice(), slice);
            }
            assert_eq!(&lex.extras.line_number, line_number);
            assert_eq!(&lex.extras.indent, indent);
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
false";
        check_lexer_output(
            input,
            &[
                (CommentSingle, Some("# single"), 1),
                (NewLine, None, 2),
                (True, None, 2),
                (CommentMulti, Some("#-\nmultiline -\nfalse #\n-#"), 5),
                (True, None, 5),
                (NewLine, None, 6),
                (False, None, 6),
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
                (String, Some(r#""hello, world!""#), 2),
                (NewLine, None, 3),
                (String, Some(r#""escaped \"\n string""#), 3),
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
f||";
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
                (Function, None, 4, 0),
                (Function, None, 4, 0),
            ],
        );
    }
}
