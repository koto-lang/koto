use crate::{Position, Span};
use std::{collections::VecDeque, iter::Peekable, ops::Range, str::Chars};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;
use unicode_xid::UnicodeXID;

/// The tokens that can emerge from the lexer
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Token {
    Error,
    Whitespace,
    NewLine,
    CommentSingle,
    CommentMulti,
    Number,
    Id,
    Wildcard,

    StringStart(StringType),
    StringEnd,
    StringLiteral,

    // Symbols
    At,
    Colon,
    Comma,
    Dot,
    Ellipsis,
    Function,
    RoundOpen,
    RoundClose,
    SquareOpen,
    SquareClose,
    CurlyOpen,
    CurlyClose,
    Range,
    RangeInclusive,

    // operators
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,

    Assign,
    AddAssign,
    SubtractAssign,
    MultiplyAssign,
    DivideAssign,
    RemainderAssign,

    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,

    Arrow,

    // Keywords
    As,
    And,
    Break,
    Catch,
    Continue,
    Debug,
    Else,
    ElseIf,
    Export,
    False,
    Finally,
    For,
    From,
    If,
    Import,
    In,
    Loop,
    Match,
    Not,
    Null,
    Or,
    Return,
    Self_,
    Switch,
    Then,
    Throw,
    True,
    Try,
    Until,
    While,
    Yield,

    // Reserved keywords
    Await,
    Const,
    Let,
}

impl Token {
    /// Returns true if the token should be counted as whitespace
    pub fn is_whitespace(&self) -> bool {
        use Token::*;
        matches!(self, Whitespace | CommentMulti | CommentSingle)
    }

    /// Returns true if the token should be counted as whitespace, including newlines
    pub fn is_whitespace_including_newline(&self) -> bool {
        self.is_whitespace() || *self == Token::NewLine
    }
}

/// The string types that the lexer can produce
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringType {
    /// A normal string
    Normal(StringQuote),
    /// A raw string
    Raw(RawStringDelimiter),
}

/// The delimiter used by a raw string
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawStringDelimiter {
    /// The quotation mark used in the raw string delimiter
    pub quote: StringQuote,
    /// The number of hashes used in the raw string delimiter
    pub hash_count: u8,
}

/// The type of quotation mark used in string delimiters
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum StringQuote {
    Double,
    Single,
}

impl TryFrom<char> for StringQuote {
    type Error = ();

    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            '"' => Ok(Self::Double),
            '\'' => Ok(Self::Single),
            _ => Err(()),
        }
    }
}

// Used to keep track of different lexing modes while working through a string
#[derive(Clone)]
enum StringMode {
    // Inside a string literal, expecting an end quote or the start of a template expression
    Literal(StringQuote),
    // Inside a string template, e.g. '{...}'
    TemplateExpr,
    // Inside an inline map in a template expression, e.g. '{foo({bar: 42})}'
    // A closing '}' will end the map rather than the template expression.
    TemplateExprInlineMap,
    // Inside formatting options for a template expression, e.g. '{...:03}'
    TemplateExprFormat,
    // The start of a raw string has just been consumed, raw string contents follow
    RawStart(RawStringDelimiter),
    // The contents of the raw string have just been consumed, the end delimiter should follow
    RawEnd(RawStringDelimiter),
}

// Separates the input source into Tokens
//
// TokenLexer is the internal implementation, KotoLexer provides the external interface.
#[derive(Clone)]
struct TokenLexer<'a> {
    // The input source
    source: &'a str,
    // The current position in the source
    current_byte: usize,
    // Used to provide the token's slice
    previous_byte: usize,
    // A cache of the previous token that was emitted
    previous_token: Option<Token>,
    // The span represented by the current token
    span: Span,
    // The indentation of the current line
    indent: usize,
    // A stack of string modes, allowing for nested mode changes while parsing strings
    string_mode_stack: Vec<StringMode>,
}

impl<'a> TokenLexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            previous_byte: 0,
            current_byte: 0,
            indent: 0,
            previous_token: None,
            span: Span::default(),
            string_mode_stack: vec![],
        }
    }

    fn source_bytes(&self) -> Range<usize> {
        self.previous_byte..self.current_byte
    }

    fn current_position(&self) -> Position {
        self.span.end
    }

    // Advance along the current line by a number of bytes
    //
    // The characters being advanced over should all be ANSI,
    // i.e. the byte count must match the character count.
    //
    // If the characters have been read as UTF-8 then advance_line_utf8 should be used instead.
    fn advance_line(&mut self, char_bytes: usize) {
        self.advance_line_utf8(char_bytes, char_bytes);
    }

    // Advance along the current line by a number of bytes, with a UTF-8 character count
    fn advance_line_utf8(&mut self, char_bytes: usize, char_count: usize) {
        // TODO, defer to advance_to_position
        self.previous_byte = self.current_byte;
        self.current_byte += char_bytes;

        let previous_end = self.span.end;
        self.span = Span {
            start: previous_end,
            end: Position {
                line: previous_end.line,
                column: previous_end.column + char_count as u32,
            },
        };
    }

    fn advance_to_position(&mut self, char_bytes: usize, position: Position) {
        self.previous_byte = self.current_byte;
        self.current_byte += char_bytes;

        self.span = Span {
            start: self.span.end,
            end: position,
        };
    }

    fn consume_newline(&mut self, mut chars: Peekable<Chars>) -> Token {
        use Token::*;

        let mut consumed_bytes = 1;

        if chars.peek() == Some(&'\r') {
            consumed_bytes += 1;
            chars.next();
        }

        match chars.next() {
            Some('\n') => {}
            _ => return Error,
        }

        self.advance_to_position(
            consumed_bytes,
            Position {
                line: self.current_position().line + 1,
                column: 0,
            },
        );

        NewLine
    }

    fn consume_comment(&mut self, mut chars: Peekable<Chars>) -> Token {
        use Token::*;

        // The # symbol has already been matched
        chars.next();

        if chars.peek() == Some(&'-') {
            // multi-line comment
            let mut char_bytes = 1;
            let mut position = self.current_position();
            position.column += 1;
            let mut end_found = false;
            while let Some(c) = chars.next() {
                char_bytes += c.len_utf8();
                position.column += c.width().unwrap_or(0) as u32;
                match c {
                    '#' => {
                        if chars.peek() == Some(&'-') {
                            chars.next();
                            char_bytes += 1;
                            position.column += 1;
                        }
                    }
                    '-' => {
                        if chars.peek() == Some(&'#') {
                            chars.next();
                            char_bytes += 1;
                            position.column += 1;
                            end_found = true;
                            break;
                        }
                    }
                    '\r' => {
                        if chars.next() != Some('\n') {
                            return Error;
                        }
                        char_bytes += 1;
                        position.line += 1;
                        position.column = 0;
                    }
                    '\n' => {
                        position.line += 1;
                        position.column = 0;
                    }
                    _ => {}
                }
            }

            self.advance_to_position(char_bytes, position);

            if end_found {
                CommentMulti
            } else {
                Error
            }
        } else {
            // single-line comment
            let (comment_bytes, comment_width) =
                consume_and_count_utf8(&mut chars, |c| !matches!(c, '\r' | '\n'));
            self.advance_line_utf8(comment_bytes + 1, comment_width + 1);
            CommentSingle
        }
    }

    fn consume_string_literal(&mut self, mut chars: Peekable<Chars>) -> Token {
        use Token::*;

        let end_quote = match self.string_mode_stack.last() {
            Some(StringMode::Literal(quote)) => *quote,
            _ => return Error,
        };

        let mut string_bytes = 0;
        let mut position = self.current_position();

        while let Some(c) = chars.peek().cloned() {
            match c {
                _ if c.try_into() == Ok(end_quote) => {
                    self.advance_to_position(string_bytes, position);
                    return StringLiteral;
                }
                '{' => {
                    self.advance_to_position(string_bytes, position);
                    // End the literal here at the start of an interpolated expression,
                    // it will be resumed after the expression
                    return StringLiteral;
                }
                '\\' => {
                    chars.next();
                    string_bytes += 1;
                    position.column += 1;

                    let skip_next_char = match chars.peek() {
                        Some('u') => {
                            chars.next();
                            string_bytes += 1;
                            position.column += 1;
                            // Skip over the start of a unicode escape sequence to avoid it being
                            // lexed as the start of an interpolated expression.
                            chars.peek() == Some(&'{')
                        }
                        Some('{') => true,
                        Some('\\') => true,
                        Some(&c) if c.try_into() == Ok(end_quote) => true,
                        _ => false,
                    };

                    if skip_next_char {
                        chars.next();
                        string_bytes += 1;
                        position.column += 1;
                    }
                }
                '\r' => {
                    chars.next();
                    if chars.next() != Some('\n') {
                        return Error;
                    }
                    string_bytes += 2;
                    position.line += 1;
                    position.column = 0;
                }
                '\n' => {
                    chars.next();
                    string_bytes += 1;
                    position.line += 1;
                    position.column = 0;
                }
                _ => {
                    chars.next();
                    string_bytes += c.len_utf8();
                    position.column += c.width().unwrap_or(0) as u32;
                }
            }
        }

        Error
    }

    fn parse_raw_string_start(&mut self, mut chars: Peekable<Chars>) -> Option<Token> {
        // look ahead and determine if this is the start of a raw string
        let mut hash_count = 0;
        loop {
            match chars.next() {
                Some('#') => {
                    hash_count += 1;
                    if hash_count == 256 {
                        break;
                    }
                }
                Some(c) => {
                    if let Ok(quote) = c.try_into() {
                        self.advance_line(2 + hash_count);
                        let hash_count = hash_count as u8;
                        self.string_mode_stack
                            .push(StringMode::RawStart(RawStringDelimiter {
                                quote,
                                hash_count,
                            }));
                        return Some(Token::StringStart(StringType::Raw(RawStringDelimiter {
                            quote,
                            hash_count,
                        })));
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }

        None
    }

    fn consume_raw_string_contents(
        &mut self,
        mut chars: Peekable<Chars>,
        delimiter: RawStringDelimiter,
    ) -> Token {
        let mut string_bytes = 0;

        let mut position = self.current_position();

        'outer: while let Some(c) = chars.next() {
            match c {
                _ if c.try_into() == Ok(delimiter.quote) => {
                    // Is this the end delimiter?
                    for i in 0..delimiter.hash_count {
                        if chars.peek() == Some(&'#') {
                            chars.next();
                        } else {
                            // Adjust for the quote and hashes that were consumed while checking if
                            // we were at the end delimiter
                            let not_the_end_delimiter_len = 1 + i as usize;
                            position.column += not_the_end_delimiter_len as u32;
                            string_bytes += not_the_end_delimiter_len;
                            // We haven't hit the required hash count, so keep consuming characters
                            // as part of the raw string's contents.
                            continue 'outer;
                        }
                    }
                    self.advance_to_position(string_bytes, position);
                    self.string_mode_stack.pop(); // StringMode::RawStart
                    self.string_mode_stack.push(StringMode::RawEnd(delimiter));
                    return Token::StringLiteral;
                }
                '\r' => {
                    if chars.next() != Some('\n') {
                        return Token::Error;
                    }
                    string_bytes += 2;
                    position.line += 1;
                    position.column = 0;
                }
                '\n' => {
                    string_bytes += 1;
                    position.line += 1;
                    position.column = 0;
                }
                _ => {
                    string_bytes += c.len_utf8();
                    position.column += c.width().unwrap_or(0) as u32;
                }
            }
        }

        Token::Error
    }

    fn consume_raw_string_end(&mut self, delimiter: RawStringDelimiter) -> Token {
        // The end delimiter has already been matched in consume_raw_string_contents,
        // so we can simply advance and return here.
        self.advance_line(1 + delimiter.hash_count as usize);
        self.string_mode_stack.pop(); // StringMode::RawEnd
        Token::StringEnd
    }

    fn consume_format_options(&mut self, input: &str) -> Token {
        // Q. Why are graphemes used to find the fill character?
        // A. Because Koto thinks of 'characters' as grapheme clusters,
        //    whereas Rust uses codepoints.
        // Q. Why not simply find the first '}' and use that as the end of the format options?
        // A. Because '}' is a valid fill character, so it needs to be checked for first.
        let mut graphemes = input.graphemes(true);
        let skip_bytes = match (graphemes.next(), graphemes.next()) {
            (Some(fill), Some("<" | "^" | ">")) => fill.len() + 1,
            _ => 0,
        };
        let Some(end_pos) = input[skip_bytes..].find('}') else {
            return Token::Error;
        };
        self.advance_line(end_pos + skip_bytes);
        self.string_mode_stack.pop(); // StringMode::TemplateExprFormat
        Token::StringLiteral
    }

    fn consume_number(&mut self, mut chars: Peekable<Chars>) -> Token {
        use Token::*;

        let has_leading_zero = chars.peek() == Some(&'0');
        let mut char_bytes = consume_and_count(&mut chars, is_digit);
        let mut allow_exponent = true;

        match chars.peek() {
            Some(&'b') if has_leading_zero && char_bytes == 1 => {
                chars.next();
                char_bytes += 1 + consume_and_count(&mut chars, is_binary_digit);
                allow_exponent = false;
            }
            Some(&'o') if has_leading_zero && char_bytes == 1 => {
                chars.next();
                char_bytes += 1 + consume_and_count(&mut chars, is_octal_digit);
                allow_exponent = false;
            }
            Some(&'x') if has_leading_zero && char_bytes == 1 => {
                chars.next();
                char_bytes += 1 + consume_and_count(&mut chars, is_hex_digit);
                allow_exponent = false;
            }
            Some(&'.') => {
                chars.next();

                match chars.peek() {
                    Some(c) if is_digit(*c) => {}
                    Some(&'e') => {
                        // lookahead to check that this isn't a function call starting with 'e'
                        // e.g. 1.exp()
                        let mut lookahead = chars.clone();
                        lookahead.next();
                        match lookahead.peek() {
                            Some(c) if is_digit(*c) => {}
                            Some(&'+' | &'-') => {}
                            _ => {
                                self.advance_line(char_bytes);
                                return Number;
                            }
                        }
                    }
                    _ => {
                        self.advance_line(char_bytes);
                        return Number;
                    }
                }

                char_bytes += 1 + consume_and_count(&mut chars, is_digit);
            }
            _ => {}
        }

        if chars.peek() == Some(&'e') && allow_exponent {
            chars.next();
            char_bytes += 1;

            if matches!(chars.peek(), Some(&'+' | &'-')) {
                chars.next();
                char_bytes += 1;
            }

            char_bytes += consume_and_count(&mut chars, is_digit);
        }

        self.advance_line(char_bytes);
        Number
    }

    fn consume_id_or_keyword(&mut self, mut chars: Peekable<Chars>) -> Token {
        use Token::*;

        // The first character has already been matched
        let c = chars.next().unwrap();

        let (char_bytes, char_count) = consume_and_count_utf8(&mut chars, is_id_continue);
        let char_bytes = c.len_utf8() + char_bytes;
        let char_count = 1 + char_count;

        let id = &self.source[self.current_byte..self.current_byte + char_bytes];

        match id {
            "else" => {
                if self
                    .source
                    .get(self.current_byte..self.current_byte + char_bytes + 3)
                    == Some("else if")
                {
                    self.advance_line(7);
                    return ElseIf;
                } else {
                    self.advance_line(4);
                    return Else;
                }
            }
            "r" => {
                if let Some(raw_string) = self.parse_raw_string_start(chars) {
                    return raw_string;
                }
            }
            _ => {}
        }

        macro_rules! check_keyword {
            ($keyword:expr, $token:ident) => {
                if id == $keyword {
                    self.advance_line($keyword.len());
                    return $token;
                }
            };
        }

        if !matches!(self.previous_token, Some(Token::Dot)) {
            check_keyword!("as", As);
            check_keyword!("and", And);
            check_keyword!("await", Await);
            check_keyword!("break", Break);
            check_keyword!("catch", Catch);
            check_keyword!("const", Const);
            check_keyword!("continue", Continue);
            check_keyword!("debug", Debug);
            check_keyword!("export", Export);
            check_keyword!("false", False);
            check_keyword!("finally", Finally);
            check_keyword!("for", For);
            check_keyword!("from", From);
            check_keyword!("if", If);
            check_keyword!("import", Import);
            check_keyword!("in", In);
            check_keyword!("let", Let);
            check_keyword!("loop", Loop);
            check_keyword!("match", Match);
            check_keyword!("not", Not);
            check_keyword!("null", Null);
            check_keyword!("or", Or);
            check_keyword!("return", Return);
            check_keyword!("self", Self_);
            check_keyword!("switch", Switch);
            check_keyword!("then", Then);
            check_keyword!("throw", Throw);
            check_keyword!("true", True);
            check_keyword!("try", Try);
            check_keyword!("until", Until);
            check_keyword!("while", While);
            check_keyword!("yield", Yield);
            check_keyword!("let", Let);
        }

        // If no keyword matched, then consume as an Id
        self.advance_line_utf8(char_bytes, char_count);
        Token::Id
    }

    fn consume_wildcard(&mut self, mut chars: Peekable<Chars>) -> Token {
        // The _ has already been matched
        let c = chars.next().unwrap();

        let (char_bytes, char_count) = consume_and_count_utf8(&mut chars, is_id_continue);
        let char_bytes = c.len_utf8() + char_bytes;
        let char_count = 1 + char_count;

        self.advance_line_utf8(char_bytes, char_count);
        Token::Wildcard
    }

    fn consume_symbol(&mut self, remaining: &str) -> Option<Token> {
        use Token::*;

        macro_rules! check_symbol {
            ($token_str:expr, $token:ident) => {
                if remaining.starts_with($token_str) {
                    self.advance_line($token_str.len());
                    return Some($token);
                }
            };
        }

        check_symbol!("...", Ellipsis);

        check_symbol!("..=", RangeInclusive);
        check_symbol!("..", Range);

        check_symbol!("->", Arrow);

        check_symbol!("==", Equal);
        check_symbol!("!=", NotEqual);
        check_symbol!(">=", GreaterOrEqual);
        check_symbol!("<=", LessOrEqual);
        check_symbol!(">", Greater);
        check_symbol!("<", Less);

        check_symbol!("=", Assign);
        check_symbol!("+=", AddAssign);
        check_symbol!("-=", SubtractAssign);
        check_symbol!("*=", MultiplyAssign);
        check_symbol!("/=", DivideAssign);
        check_symbol!("%=", RemainderAssign);

        check_symbol!("+", Add);
        check_symbol!("-", Subtract);
        check_symbol!("*", Multiply);
        check_symbol!("/", Divide);
        check_symbol!("%", Remainder);

        check_symbol!("@", At);
        check_symbol!(":", Colon);
        check_symbol!(",", Comma);
        check_symbol!(".", Dot);
        check_symbol!("(", RoundOpen);
        check_symbol!(")", RoundClose);
        check_symbol!("|", Function);
        check_symbol!("[", SquareOpen);
        check_symbol!("]", SquareClose);
        check_symbol!("{", CurlyOpen);
        check_symbol!("}", CurlyClose);

        None
    }

    fn get_next_token(&mut self) -> Option<Token> {
        use Token::*;

        let result = match self.source.get(self.current_byte..) {
            Some(remaining) if !remaining.is_empty() => {
                if self.previous_token == Some(Token::NewLine) {
                    // Reset the indent after a newline.
                    // If whitespace follows then the indent will be increased.
                    self.indent = 0;
                }

                let mut chars = remaining.chars().peekable();
                let next_char = *chars.peek().unwrap(); // At least one char is remaining

                let string_mode = self.string_mode_stack.last().cloned();

                let result = match string_mode {
                    Some(StringMode::Literal(quote)) => match next_char {
                        c if c.try_into() == Ok(quote) => {
                            self.advance_line(1);
                            self.string_mode_stack.pop();
                            StringEnd
                        }
                        '{' => {
                            self.advance_line(1);
                            self.string_mode_stack.push(StringMode::TemplateExpr);
                            CurlyOpen
                        }
                        _ => self.consume_string_literal(chars),
                    },
                    Some(StringMode::RawStart(delimiter)) => {
                        self.consume_raw_string_contents(chars, delimiter)
                    }
                    Some(StringMode::RawEnd(delimiter)) => self.consume_raw_string_end(delimiter),
                    Some(StringMode::TemplateExprFormat) => self.consume_format_options(remaining),
                    _ => match next_char {
                        c if is_whitespace(c) => {
                            let count = consume_and_count(&mut chars, is_whitespace);
                            self.advance_line(count);
                            if matches!(self.previous_token, Some(Token::NewLine) | None) {
                                self.indent = count;
                            }
                            Whitespace
                        }
                        '\r' | '\n' => self.consume_newline(chars),
                        '#' => self.consume_comment(chars),
                        '"' => {
                            self.advance_line(1);
                            self.string_mode_stack
                                .push(StringMode::Literal(StringQuote::Double));
                            StringStart(StringType::Normal(StringQuote::Double))
                        }
                        '\'' => {
                            self.advance_line(1);
                            self.string_mode_stack
                                .push(StringMode::Literal(StringQuote::Single));
                            StringStart(StringType::Normal(StringQuote::Single))
                        }
                        '0'..='9' => self.consume_number(chars),
                        c if is_id_start(c) => self.consume_id_or_keyword(chars),
                        '_' => self.consume_wildcard(chars),
                        _ => {
                            let result = match self.consume_symbol(remaining) {
                                Some(result) => result,
                                None => {
                                    self.advance_line(1);
                                    Error
                                }
                            };

                            use StringMode::*;
                            match (result, string_mode) {
                                (CurlyOpen, Some(TemplateExpr)) => {
                                    self.string_mode_stack.push(TemplateExprInlineMap);
                                }
                                (Colon, Some(TemplateExpr)) => {
                                    self.string_mode_stack.push(TemplateExprFormat);
                                }
                                (CurlyClose, Some(TemplateExpr | TemplateExprInlineMap)) => {
                                    self.string_mode_stack.pop();
                                }
                                _ => {}
                            }

                            result
                        }
                    },
                };

                Some(result)
            }
            _ => None,
        };

        self.previous_token = result;
        result
    }
}

impl<'a> Iterator for TokenLexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.get_next_token()
    }
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_binary_digit(c: char) -> bool {
    matches!(c, '0' | '1')
}

fn is_octal_digit(c: char) -> bool {
    matches!(c, '0'..='7')
}

fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// Returns true if the character matches the XID_Start Unicode property
pub fn is_id_start(c: char) -> bool {
    UnicodeXID::is_xid_start(c)
}

/// Returns true if the character matches the XID_Continue Unicode property
pub fn is_id_continue(c: char) -> bool {
    UnicodeXID::is_xid_continue(c)
}

fn consume_and_count(chars: &mut Peekable<Chars>, predicate: impl Fn(char) -> bool) -> usize {
    let mut char_bytes = 0;

    while let Some(c) = chars.peek() {
        if !predicate(*c) {
            break;
        }
        char_bytes += 1;
        chars.next();
    }

    char_bytes
}

fn consume_and_count_utf8(
    chars: &mut Peekable<Chars>,
    predicate: impl Fn(char) -> bool,
) -> (usize, usize) {
    let mut char_bytes = 0;
    let mut char_count = 0;

    while let Some(c) = chars.peek() {
        if !predicate(*c) {
            break;
        }
        char_bytes += c.len_utf8();
        char_count += c.width().unwrap_or(0);
        chars.next();
    }

    (char_bytes, char_count)
}

/// A [Token] along with additional metadata
#[derive(Clone, PartialEq, Debug)]
pub struct LexedToken {
    /// The token
    pub token: Token,
    /// The byte positions in the source representing the token
    pub source_bytes: Range<usize>,
    /// The token's span
    pub span: Span,
    /// The indentation level of the token's starting line
    pub indent: usize,
}

impl LexedToken {
    /// A helper for getting the token's starting line
    pub fn line(&self) -> u32 {
        self.span.start.line
    }

    /// A helper for getting the token's string slice from the source
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.source_bytes.clone()]
    }
}

impl Default for LexedToken {
    fn default() -> Self {
        Self {
            token: Token::Error,
            source_bytes: Default::default(),
            span: Default::default(),
            indent: Default::default(),
        }
    }
}

/// The lexer used by the Koto parser
///
/// Wraps a TokenLexer with unbounded lookahead, see peek_n().
#[derive(Clone)]
pub struct KotoLexer<'a> {
    lexer: TokenLexer<'a>,
    token_queue: VecDeque<LexedToken>,
}

impl<'a> KotoLexer<'a> {
    /// Initializes a lexer with the given input script
    pub fn new(source: &'a str) -> Self {
        Self {
            lexer: TokenLexer::new(source),
            token_queue: VecDeque::new(),
        }
    }

    /// Returns the input source
    pub fn source(&self) -> &'a str {
        self.lexer.source
    }

    /// Peeks the nth token that will appear in the output stream
    ///
    /// peek_n(0) is equivalent to calling peek().
    /// peek_n(1) returns the token that will appear after that, and so forth.
    pub fn peek(&mut self, n: usize) -> Option<&LexedToken> {
        let token_queue_len = self.token_queue.len();
        let tokens_to_add = token_queue_len + 1 - n.max(token_queue_len);

        for _ in 0..tokens_to_add {
            if let Some(next) = self.next_token() {
                self.token_queue.push_back(next);
            } else {
                break;
            }
        }

        self.token_queue.get(n)
    }

    fn next_token(&mut self) -> Option<LexedToken> {
        self.lexer.next().map(|token| LexedToken {
            token,
            source_bytes: self.lexer.source_bytes(),
            span: self.lexer.span,
            indent: self.lexer.indent,
        })
    }
}

impl<'a> Iterator for KotoLexer<'a> {
    type Item = LexedToken;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.token_queue.pop_front() {
            Some(next)
        } else {
            self.next_token()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod lexer_output {
        use super::{Token::*, *};

        fn check_lexer_output(source: &str, tokens: &[(Token, Option<&str>, u32)]) {
            let mut lex = KotoLexer::new(source);

            for (i, (token, maybe_slice, line_number)) in tokens.iter().enumerate() {
                loop {
                    match lex.next().expect("Expected token") {
                        LexedToken {
                            token: Whitespace, ..
                        } => continue,
                        output => {
                            assert_eq!(*token, output.token, "Token mismatch at position {i}");
                            if let Some(slice) = maybe_slice {
                                assert_eq!(
                                    *slice,
                                    output.slice(source),
                                    "Slice mismatch at position {i}"
                                );
                            }
                            assert_eq!(
                                *line_number,
                                output.line(),
                                "Line number mismatch at position {i}",
                            );
                            break;
                        }
                    }
                }
            }

            assert_eq!(lex.next(), None);
        }

        fn check_lexer_output_indented(source: &str, tokens: &[(Token, Option<&str>, u32, u32)]) {
            let mut lex = KotoLexer::new(source);

            for (i, (token, maybe_slice, line_number, indent)) in tokens.iter().enumerate() {
                loop {
                    match lex.next().expect("Expected token") {
                        LexedToken {
                            token: Whitespace, ..
                        } => continue,
                        output => {
                            assert_eq!(output.token, *token, "Mismatch at token {i}");
                            if let Some(slice) = maybe_slice {
                                assert_eq!(output.slice(source), *slice, "Mismatch at token {i}");
                            }
                            assert_eq!(
                                output.line(),
                                *line_number,
                                "Line number - expected: {}, actual: {} - (token {i} - {token:?})",
                                *line_number,
                                output.line(),
                            );
                            assert_eq!(
                                output.indent as u32, *indent,
                                "Indent (token {i} - {token:?})"
                            );
                            break;
                        }
                    }
                }
            }

            assert_eq!(lex.next(), None);
        }

        fn normal_string(quote: StringQuote) -> Token {
            Token::StringStart(StringType::Normal(quote))
        }

        fn raw_string(quote: StringQuote, hash_count: u8) -> Token {
            Token::StringStart(StringType::Raw(RawStringDelimiter { quote, hash_count }))
        }

        #[test]
        fn ids() {
            let input = "id id1 id_2 i_d_3 ïd_ƒôûr if iff _ _foo";
            check_lexer_output(
                input,
                &[
                    (Id, Some("id"), 0),
                    (Id, Some("id1"), 0),
                    (Id, Some("id_2"), 0),
                    (Id, Some("i_d_3"), 0),
                    (Id, Some("ïd_ƒôûr"), 0),
                    (If, None, 0),
                    (Id, Some("iff"), 0),
                    (Wildcard, Some("_"), 0),
                    (Wildcard, Some("_foo"), 0),
                ],
            );
        }

        #[test]
        fn indent() {
            let input = "\
if true then
  foo 1

bar 2";
            check_lexer_output_indented(
                input,
                &[
                    (If, None, 0, 0),
                    (True, None, 0, 0),
                    (Then, None, 0, 0),
                    (NewLine, None, 0, 0),
                    (Id, Some("foo"), 1, 2),
                    (Number, Some("1"), 1, 2),
                    (NewLine, None, 1, 2),
                    (NewLine, None, 2, 0),
                    (Id, Some("bar"), 3, 0),
                    (Number, Some("2"), 3, 0),
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
                    (CommentSingle, Some("# single"), 0),
                    (NewLine, None, 0),
                    (True, None, 1),
                    (CommentMulti, Some("#-\nmultiline -\nfalse #\n-#"), 1),
                    (True, None, 4),
                    (NewLine, None, 4),
                    (RoundOpen, None, 5),
                    (RoundClose, None, 5),
                ],
            );
        }

        #[test]
        fn strings() {
            let input = r#"
"hello, world!"
"escaped \\\"\n\{ string"
"double-\"quoted\" 'string'"
'single-\'quoted\' "string"'
""
"\\"
"#;

            use StringQuote::*;
            check_lexer_output(
                input,
                &[
                    (NewLine, None, 0),
                    (normal_string(Double), None, 1),
                    (StringLiteral, Some("hello, world!"), 1),
                    (StringEnd, None, 1),
                    (NewLine, None, 1),
                    (normal_string(Double), None, 2),
                    (StringLiteral, Some(r#"escaped \\\"\n\{ string"#), 2),
                    (StringEnd, None, 2),
                    (NewLine, None, 2),
                    (normal_string(Double), None, 3),
                    (StringLiteral, Some(r#"double-\"quoted\" 'string'"#), 3),
                    (StringEnd, None, 3),
                    (NewLine, None, 3),
                    (normal_string(Single), None, 4),
                    (StringLiteral, Some(r#"single-\'quoted\' "string""#), 4),
                    (StringEnd, None, 4),
                    (NewLine, None, 4),
                    (normal_string(Double), None, 5),
                    (StringEnd, None, 5),
                    (NewLine, None, 5),
                    (normal_string(Double), None, 6),
                    (StringLiteral, Some(r"\\"), 6),
                    (StringEnd, None, 6),
                    (NewLine, None, 6),
                ],
            );
        }

        #[test]
        fn raw_strings() {
            let input = r#"
r"{foo}"
r#''bar''#
"#;

            check_lexer_output(
                input,
                &[
                    (NewLine, None, 0),
                    (raw_string(StringQuote::Double, 0), None, 1),
                    (StringLiteral, Some("{foo}"), 1),
                    (StringEnd, None, 1),
                    (NewLine, None, 1),
                    (raw_string(StringQuote::Single, 1), None, 2),
                    (StringLiteral, Some("'bar'"), 2),
                    (StringEnd, None, 2),
                    (NewLine, None, 2),
                ],
            );
        }

        #[test]
        fn interpolated_string_ids() {
            let input = r#"
"hello {name}, how are you?"
'{foo}{bar}'
"#;
            use StringQuote::*;
            check_lexer_output(
                input,
                &[
                    (NewLine, None, 0),
                    (normal_string(Double), None, 1),
                    (StringLiteral, Some("hello "), 1),
                    (CurlyOpen, None, 1),
                    (Id, Some("name"), 1),
                    (CurlyClose, None, 1),
                    (StringLiteral, Some(", how are you?"), 1),
                    (StringEnd, None, 1),
                    (NewLine, None, 1),
                    (normal_string(Single), None, 2),
                    (CurlyOpen, None, 2),
                    (Id, Some("foo"), 2),
                    (CurlyClose, None, 2),
                    (CurlyOpen, None, 2),
                    (Id, Some("bar"), 2),
                    (CurlyClose, None, 2),
                    (StringEnd, None, 2),
                    (NewLine, None, 2),
                ],
            );
        }

        #[test]
        fn interpolated_string_expressions() {
            let input = r#"
"x + y == {x + y}"
'{'\{foo}'}'
"#;
            use StringQuote::*;
            check_lexer_output(
                input,
                &[
                    (NewLine, None, 0),
                    (normal_string(Double), None, 1),
                    (StringLiteral, Some("x + y == "), 1),
                    (CurlyOpen, None, 1),
                    (Id, Some("x"), 1),
                    (Add, None, 1),
                    (Id, Some("y"), 1),
                    (CurlyClose, None, 1),
                    (StringEnd, None, 1),
                    (NewLine, None, 1),
                    (normal_string(Single), None, 2),
                    (CurlyOpen, None, 2),
                    (normal_string(Single), None, 2),
                    (StringLiteral, Some("\\{foo}"), 2),
                    (StringEnd, None, 2),
                    (CurlyClose, None, 2),
                    (StringEnd, None, 2),
                    (NewLine, None, 2),
                ],
            );
        }

        #[test]
        fn interpolated_string_format_options() {
            let input = r#"
'{a + b:_^3.4}'
"#;
            use StringQuote::*;
            check_lexer_output(
                input,
                &[
                    (NewLine, None, 0),
                    (normal_string(Single), None, 1),
                    (CurlyOpen, None, 1),
                    (Id, Some("a"), 1),
                    (Add, None, 1),
                    (Id, Some("b"), 1),
                    (Colon, None, 1),
                    (StringLiteral, Some("_^3.4"), 1),
                    (CurlyClose, None, 1),
                    (StringEnd, None, 1),
                    (NewLine, None, 1),
                ],
            );
        }

        #[test]
        fn operators() {
            let input = "> >= -> < <=";

            check_lexer_output(
                input,
                &[
                    (Greater, None, 0),
                    (GreaterOrEqual, None, 0),
                    (Arrow, None, 0),
                    (Less, None, 0),
                    (LessOrEqual, None, 0),
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
-8e8
0xabadcafe
0xABADCAFE
0o707606
0b1010101";
            check_lexer_output(
                input,
                &[
                    (Number, Some("123"), 0),
                    (NewLine, None, 0),
                    (Number, Some("55.5"), 1),
                    (NewLine, None, 1),
                    (Subtract, None, 2),
                    (Number, Some("1e-3"), 2),
                    (NewLine, None, 2),
                    (Number, Some("0.5e+9"), 3),
                    (NewLine, None, 3),
                    (Subtract, None, 4),
                    (Number, Some("8e8"), 4),
                    (NewLine, None, 4),
                    (Number, Some("0xabadcafe"), 5),
                    (NewLine, None, 5),
                    (Number, Some("0xABADCAFE"), 6),
                    (NewLine, None, 6),
                    (Number, Some("0o707606"), 7),
                    (NewLine, None, 7),
                    (Number, Some("0b1010101"), 8),
                ],
            );
        }

        #[test]
        fn accesses_on_numbers() {
            let input = "\
1.0.sin()
-1e-3.abs()
1.min x
9.exp()";
            check_lexer_output(
                input,
                &[
                    (Number, Some("1.0"), 0),
                    (Dot, None, 0),
                    (Id, Some("sin"), 0),
                    (RoundOpen, None, 0),
                    (RoundClose, None, 0),
                    (NewLine, None, 0),
                    (Subtract, None, 1),
                    (Number, Some("1e-3"), 1),
                    (Dot, None, 1),
                    (Id, Some("abs"), 1),
                    (RoundOpen, None, 1),
                    (RoundClose, None, 1),
                    (NewLine, None, 1),
                    (Number, Some("1"), 2),
                    (Dot, None, 2),
                    (Id, Some("min"), 2),
                    (Id, Some("x"), 2),
                    (NewLine, None, 2),
                    (Number, Some("9"), 3),
                    (Dot, None, 3),
                    (Id, Some("exp"), 3),
                    (RoundOpen, None, 3),
                    (RoundClose, None, 3),
                ],
            );
        }

        #[test]
        fn compound_assignment() {
            let input = "\
a += 1
b -= 2
c *= 3";
            check_lexer_output(
                input,
                &[
                    (Id, Some("a"), 0),
                    (AddAssign, None, 0),
                    (Number, Some("1"), 0),
                    (NewLine, None, 0),
                    (Id, Some("b"), 1),
                    (SubtractAssign, None, 1),
                    (Number, Some("2"), 1),
                    (NewLine, None, 1),
                    (Id, Some("c"), 2),
                    (MultiplyAssign, None, 2),
                    (Number, Some("3"), 2),
                ],
            );
        }

        #[test]
        fn let_expression() {
            let input = "let my_var: Number = 42";

            check_lexer_output(
                input,
                &[
                    (Let, None, 0),
                    (Id, Some("my_var"), 0),
                    (Colon, None, 0),
                    (Id, Some("Number"), 0),
                    (Assign, None, 0),
                    (Number, Some("42"), 0),
                ],
            )
        }

        #[test]
        fn ranges() {
            let input = "\
a[..=9]
x = [i for i in 0..5]";
            check_lexer_output(
                input,
                &[
                    (Id, Some("a"), 0),
                    (SquareOpen, None, 0),
                    (RangeInclusive, None, 0),
                    (Number, Some("9"), 0),
                    (SquareClose, None, 0),
                    (NewLine, None, 0),
                    (Id, Some("x"), 1),
                    (Assign, None, 1),
                    (SquareOpen, None, 1),
                    (Id, Some("i"), 1),
                    (For, None, 1),
                    (Id, Some("i"), 1),
                    (In, None, 1),
                    (Number, Some("0"), 1),
                    (Range, None, 1),
                    (Number, Some("5"), 1),
                    (SquareClose, None, 1),
                ],
            );
        }

        #[test]
        fn function() {
            let input = "\
export f = |a, b...|
  c = a + b.size()
  c
f()";
            check_lexer_output_indented(
                input,
                &[
                    (Export, None, 0, 0),
                    (Id, Some("f"), 0, 0),
                    (Assign, None, 0, 0),
                    (Function, None, 0, 0),
                    (Id, Some("a"), 0, 0),
                    (Comma, None, 0, 0),
                    (Id, Some("b"), 0, 0),
                    (Ellipsis, None, 0, 0),
                    (Function, None, 0, 0),
                    (NewLine, None, 0, 0),
                    (Id, Some("c"), 1, 2),
                    (Assign, None, 1, 2),
                    (Id, Some("a"), 1, 2),
                    (Add, None, 1, 2),
                    (Id, Some("b"), 1, 2),
                    (Dot, None, 1, 2),
                    (Id, Some("size"), 1, 2),
                    (RoundOpen, None, 1, 2),
                    (RoundClose, None, 1, 2),
                    (NewLine, None, 1, 2),
                    (Id, Some("c"), 2, 2),
                    (NewLine, None, 2, 2),
                    (Id, Some("f"), 3, 0),
                    (RoundOpen, None, 3, 0),
                    (RoundClose, None, 3, 0),
                ],
            );
        }

        #[test]
        fn if_inline() {
            let input = "1 + if true then 0 else 1";
            check_lexer_output(
                input,
                &[
                    (Number, Some("1"), 0),
                    (Add, None, 0),
                    (If, None, 0),
                    (True, None, 0),
                    (Then, None, 0),
                    (Number, Some("0"), 0),
                    (Else, None, 0),
                    (Number, Some("1"), 0),
                ],
            );
        }

        #[test]
        fn if_block() {
            let input = "\
if true
  0
else if false
  1
else
  0";
            check_lexer_output_indented(
                input,
                &[
                    (If, None, 0, 0),
                    (True, None, 0, 0),
                    (NewLine, None, 0, 0),
                    (Number, Some("0"), 1, 2),
                    (NewLine, None, 1, 2),
                    (ElseIf, None, 2, 0),
                    (False, None, 2, 0),
                    (NewLine, None, 2, 0),
                    (Number, Some("1"), 3, 2),
                    (NewLine, None, 3, 2),
                    (Else, None, 4, 0),
                    (NewLine, None, 4, 0),
                    (Number, Some("0"), 5, 2),
                ],
            );
        }

        #[test]
        fn map_access() {
            let input = "m.检验.foo[1].bär()";

            check_lexer_output(
                input,
                &[
                    (Id, Some("m"), 0),
                    (Dot, None, 0),
                    (Id, Some("检验"), 0),
                    (Dot, None, 0),
                    (Id, Some("foo"), 0),
                    (SquareOpen, None, 0),
                    (Number, Some("1"), 0),
                    (SquareClose, None, 0),
                    (Dot, None, 0),
                    (Id, Some("bär"), 0),
                    (RoundOpen, None, 0),
                    (RoundClose, None, 0),
                ],
            );
        }

        #[test]
        fn map_access_with_keyword_as_key() {
            let input = "foo.and()";

            check_lexer_output(
                input,
                &[
                    (Id, Some("foo"), 0),
                    (Dot, None, 0),
                    (Id, Some("and"), 0),
                    (RoundOpen, None, 0),
                    (RoundClose, None, 0),
                ],
            );
        }

        #[test]
        fn windows_line_endings() {
            let input = "123\r\n456\r\n789";

            check_lexer_output(
                input,
                &[
                    (Number, Some("123"), 0),
                    (NewLine, None, 0),
                    (Number, Some("456"), 1),
                    (NewLine, None, 1),
                    (Number, Some("789"), 2),
                ],
            );
        }
    }

    mod peek {
        use super::*;

        #[test]
        fn map_access_in_list() {
            let source = "
[foo.bar]
";
            let mut lex = KotoLexer::new(source);
            assert_eq!(lex.peek(0).unwrap().token, Token::NewLine);
            assert_eq!(lex.peek(1).unwrap().token, Token::SquareOpen);
            assert_eq!(lex.peek(2).unwrap().token, Token::Id);
            assert_eq!(lex.peek(2).unwrap().slice(source), "foo");
            assert_eq!(lex.peek(3).unwrap().token, Token::Dot);
            assert_eq!(lex.peek(4).unwrap().token, Token::Id);
            assert_eq!(lex.peek(4).unwrap().slice(source), "bar");
            assert_eq!(lex.peek(5).unwrap().token, Token::SquareClose);
            assert_eq!(lex.peek(6).unwrap().token, Token::NewLine);
            assert_eq!(lex.peek(7), None);
        }

        #[test]
        fn multiline_chain() {
            let source = "
x.iter()
  .skip 1
";
            let mut lex = KotoLexer::new(source);
            assert_eq!(lex.peek(0).unwrap().token, Token::NewLine);
            assert_eq!(lex.peek(1).unwrap().token, Token::Id);
            assert_eq!(lex.peek(1).unwrap().slice(source), "x");
            assert_eq!(lex.peek(2).unwrap().token, Token::Dot);
            assert_eq!(lex.peek(3).unwrap().token, Token::Id);
            assert_eq!(lex.peek(3).unwrap().slice(source), "iter");
            assert_eq!(lex.peek(4).unwrap().token, Token::RoundOpen);
            assert_eq!(lex.peek(5).unwrap().token, Token::RoundClose);
            assert_eq!(lex.peek(6).unwrap().token, Token::NewLine);
            assert_eq!(lex.peek(7).unwrap().token, Token::Whitespace);
            assert_eq!(lex.peek(8).unwrap().token, Token::Dot);
            assert_eq!(lex.peek(9).unwrap().token, Token::Id);
            assert_eq!(lex.peek(9).unwrap().slice(source), "skip");
            assert_eq!(lex.peek(10).unwrap().token, Token::Whitespace);
            assert_eq!(lex.peek(11).unwrap().token, Token::Number);
            assert_eq!(lex.peek(12).unwrap().token, Token::NewLine);
            assert_eq!(lex.peek(13), None);
        }
    }
}
