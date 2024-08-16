use unicode_xid::UnicodeXID;

use std::{fmt, rc::Rc};

use crate::{
    diagnostic::Diagnostic,
    parser::{
        ForkableCursor, LookBackParseCursor, LookaheadResult, OptionParser, ParseContext,
        ParseCursor, ParseHinter, ParseSequence, StrCursor, StrSequence,
    },
    span::Span,
    symbol::Symbol,
};

// === Tokens === //

#[derive(Debug, Clone)]
pub enum Token {
    Group(TokenGroup),
    StringLit(TokenStringLit),
    CharLit(TokenCharLit),
    NumberLit(TokenNumberLit),
    Ident(TokenIdent),
    Punct(TokenPunct),
}

impl Token {
    pub fn span(&self) -> Span {
        match self {
            Token::Group(group) => group.span(),
            Token::StringLit(lit) => lit.span,
            Token::CharLit(lit) => lit.span,
            Token::NumberLit(lit) => lit.span,
            Token::Ident(ident) => ident.span,
            Token::Punct(punct) => punct.span,
        }
    }

    pub fn as_group(&self) -> Option<&TokenGroup> {
        match self {
            Self::Group(token) => Some(token),
            _ => None,
        }
    }

    pub fn as_string_lit(&self) -> Option<&TokenStringLit> {
        match self {
            Self::StringLit(token) => Some(token),
            _ => None,
        }
    }

    pub fn as_char_lit(&self) -> Option<&TokenCharLit> {
        match self {
            Self::CharLit(token) => Some(token),
            _ => None,
        }
    }

    pub fn as_number_lit(&self) -> Option<&TokenNumberLit> {
        match self {
            Self::NumberLit(token) => Some(token),
            _ => None,
        }
    }

    pub fn as_ident(&self) -> Option<&TokenIdent> {
        match self {
            Self::Ident(token) => Some(token),
            _ => None,
        }
    }

    pub fn as_punct(&self) -> Option<&TokenPunct> {
        match self {
            Self::Punct(token) => Some(token),
            _ => None,
        }
    }
}

impl From<TokenGroup> for Token {
    fn from(value: TokenGroup) -> Self {
        Self::Group(value)
    }
}

impl From<TokenStringLit> for Token {
    fn from(value: TokenStringLit) -> Self {
        Self::StringLit(value)
    }
}

impl From<TokenCharLit> for Token {
    fn from(value: TokenCharLit) -> Self {
        Self::CharLit(value)
    }
}

impl From<TokenNumberLit> for Token {
    fn from(value: TokenNumberLit) -> Self {
        Self::NumberLit(value)
    }
}

impl From<TokenIdent> for Token {
    fn from(value: TokenIdent) -> Self {
        Self::Ident(value)
    }
}

impl From<TokenPunct> for Token {
    fn from(value: TokenPunct) -> Self {
        Self::Punct(value)
    }
}

// Group
#[derive(Debug, Clone)]
pub struct TokenGroup {
    pub open: Span,
    pub close: Span,
    pub delimiter: GroupDelimiter,
    pub tokens: Rc<[Token]>,
}

impl TokenGroup {
    pub fn span(&self) -> Span {
        self.open.to(self.close)
    }

    pub fn cursor(&self) -> TokenCursor<'_> {
        TokenCursor {
            prev_span: self.open,
            close_span: self.close,
            iter: self.tokens.iter(),
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum GroupDelimiter {
    Brace,
    Bracket,
    Paren,
    File,
}

impl GroupDelimiter {
    pub fn closer(self) -> &'static str {
        match self {
            GroupDelimiter::Brace => "}",
            GroupDelimiter::Bracket => "]",
            GroupDelimiter::Paren => ")",
            GroupDelimiter::File => "end of file",
        }
    }
}

// StringLit
#[derive(Debug, Copy, Clone)]
pub struct TokenStringLit {
    pub span: Span,
    pub inner: Symbol,
}

// CharLit
#[derive(Debug, Copy, Clone)]
pub struct TokenCharLit {
    pub span: Span,
    pub ch: char,
}

// NumberLit
#[derive(Debug, Copy, Clone)]
pub struct TokenNumberLit {
    pub span: Span,
    pub data: Symbol,
}

// Ident
#[derive(Debug, Copy, Clone)]
pub struct TokenIdent {
    pub span: Span,
    pub text: Symbol,
    pub is_raw: bool,
}

// Puncts
#[derive(Debug, Copy, Clone)]
pub struct TokenPunct {
    pub span: Span,
    pub char: PunctChar,
    pub glued: bool,
}

macro_rules! define_puncts {
	(
		$(#[$attr:meta])*
		$vis:vis enum $enum_name:ident {
			$($name:ident = $char:expr),*
			$(,)?
		}
	) => {
		$(#[$attr])*
		#[derive(Copy, Clone, Hash, Eq, PartialEq)]
		$vis enum $enum_name {
			$($name),*
		}

		impl $enum_name {
			pub const fn from_char(c: char) -> Option<Self> {
				match c {
					$($char => Some(Self::$name),)*
					_ => None,
				}
			}

			pub const fn to_char(self) -> char {
				match self {
					$(Self::$name => $char),*
				}
			}

			pub fn as_char_name(self) -> Symbol {
				const NAMES: [&str; 0 $(+ {let _ = $char; 1})*] = [
					$(concat!("`", $char, "`"),)*
				];

				Symbol::new_static(NAMES[self as usize])
			}
		}

		impl fmt::Debug for $enum_name {
			fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
				write!(f, "{:?}", self.to_char())
			}
		}
	};
}

define_puncts! {
    pub enum PunctChar {
        Equals = '=',
        LessThan = '<',
        GreaterThan = '>',
        Exclamation = '!',
        Tilde = '~',
        Plus = '+',
        Minus = '-',
        Asterisk = '*',
        Slash = '/',
        Percent = '%',
        Caret = '^',
        Ampersand = '&',
        Bar = '|',
        At = '@',
        Point = '.',
        Comma = ',',
        Semicolon = ';',
        Colon = ':',
        Pound = '#',
        Dollar = '$',
        Question = '?',
        BackSlash = '\\',
        BackTick = '`',

    }
}

#[doc(hidden)]
pub mod punct_macro_internals {
    pub use {
        super::PunctChar,
        std::{concat, option::Option::*, panic},
    };
}

#[macro_export]
macro_rules! punct {
    ($ch:expr) => {{
        const CHAR: $crate::tokens::punct_macro_internals::PunctChar =
            match $crate::tokens::punct_macro_internals::PunctChar::from_char($ch) {
                $crate::tokens::punct_macro_internals::Some(v) => v,
                $crate::tokens::punct_macro_internals::None => {
                    $crate::tokens::punct_macro_internals::panic!(
                        $crate::tokens::punct_macro_internals::concat!("unknown punct `", $ch, "`"),
                    );
                }
            };
        CHAR
    }};
}

pub use punct;

// === Tokenizer === //

pub fn tokenize(file: Span) -> TokenGroup {
    let cx = ParseContext::new();
    {
        let mut c = cx.enter(StrCursor::new_span(file));
        let open_span = Span::new(c.next_span().start, c.next_span().start);
        parse_group(&mut c, open_span, GroupDelimiter::File)
    }
}

fn parse_group(c: &mut StrSequence, open_span: Span, delimiter: GroupDelimiter) -> TokenGroup {
    let mut tokens = Vec::new();

    let _wp = 'wp_ctor: {
        Some(c.while_parsing(match delimiter {
            GroupDelimiter::Brace => Symbol::new_static("braced token group"),
            GroupDelimiter::Bracket => Symbol::new_static("bracketed token group"),
            GroupDelimiter::Paren => Symbol::new_static("parenthesized token group"),
            GroupDelimiter::File => break 'wp_ctor None,
        }))
    };

    #[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
    enum ParseMode {
        AfterPunct,
        AfterNumeric,
        #[default]
        Normal,
    }

    let mut parse_mode = ParseMode::Normal;

    let close_span = loop {
        let curr_mode = std::mem::take(&mut parse_mode);
        let curr_start = c.next_span();

        // Match whitespace
        if c.expect(Symbol::new_static("whitespace"), |c| {
            c.next().is_some_and(char::is_whitespace)
        }) {
            continue;
        }

        // Match comments
        if parse_line_comment(c) {
            continue;
        }

        if parse_block_comment(c) {
            continue;
        }

        // Match opening delimiters
        if let Some(delimiter) = parse_open_delimiter(c) {
            tokens.push(parse_group(c, curr_start, delimiter).into());
            continue;
        }

        // Match closing delimiters
        if let Some(close_delimiter) = parse_close_delimiter(c, delimiter) {
            if delimiter != close_delimiter {
                let prefix = if close_delimiter == GroupDelimiter::File {
                    "unclosed delimiter"
                } else {
                    "mismatched delimiter"
                };

                c.error(
                    {
                        let mut diagnostic = Diagnostic::span_err(
                            curr_start,
                            format!("{prefix}; expected {}", delimiter.closer()),
                        );

                        if delimiter != GroupDelimiter::File {
                            diagnostic.subs.push(Diagnostic::span_info(
                                open_span,
                                "looking for match to this delimiter",
                            ));
                        }

                        diagnostic
                    },
                    |_| (), // Already caught up
                );
            }

            break curr_start;
        }

        // Match string literals
        if let Some(sl) = parse_string_literal(c) {
            tokens.push(sl.into());
            continue;
        }

        // Match character literals
        if let Some(cl) = parse_char_literal(c) {
            tokens.push(cl.into());
            continue;
        }

        // Match numeric literals
        if curr_mode != ParseMode::AfterNumeric {
            if let Some(nl) = parse_numeric_literal(c) {
                tokens.push(nl.into());
                parse_mode = ParseMode::AfterNumeric;
                continue;
            }
        }

        // Match raw identifier or punctuation
        if c.expect(Symbol::new_static("`@`"), |c| c.next() == Some('@')) {
            // Match as raw identifier
            if let Some(ident) = parse_ident(c, curr_start, true) {
                tokens.push(ident.into());
                continue;
            }

            // Otherwise, treat as punctuation.
            tokens.push(
                TokenPunct {
                    span: curr_start,
                    char: punct!('@'),
                    glued: curr_mode == ParseMode::AfterPunct,
                }
                .into(),
            );
            parse_mode = ParseMode::AfterPunct;
            continue;
        }

        // Match ident
        if curr_mode != ParseMode::AfterNumeric {
            if let Some(ident) = parse_ident(c, curr_start, false) {
                tokens.push(ident.into());
                continue;
            }
        }

        // Match punctuation
        if let Some(char) = parse_punct_char(c, curr_mode != ParseMode::AfterNumeric) {
            tokens.push(
                TokenPunct {
                    span: curr_start,
                    char,
                    glued: curr_mode == ParseMode::AfterPunct,
                }
                .into(),
            );
            parse_mode = ParseMode::AfterPunct;
            continue;
        }

        // Otherwise, we're stuck.
        c.stuck(|c| {
            // Just ignore the offending character.
            let _ = c.next();
        });
    };

    TokenGroup {
        open: open_span,
        close: close_span,
        delimiter,
        tokens: Rc::from_iter(tokens),
    }
}

fn parse_line_comment(c: &mut StrSequence) -> bool {
    if !c.expect(Symbol::new_static("`//`"), |c| {
        c.next() == Some('/') && c.next() == Some('/')
    }) {
        return false;
    }

    loop {
        let read = c.irrefutable(|c| c.next());
        if read.is_none() || read == Some('\n') {
            break;
        }
    }

    true
}

fn parse_block_comment(c: &mut StrSequence) -> bool {
    if !c.expect(Symbol::new_static("`/*`"), |c| {
        c.next() == Some('/') && c.next() == Some('*')
    }) {
        return false;
    }

    let _wp = c.while_parsing(Symbol::new_static("block comment"));

    loop {
        if c.expect(Symbol::new_static("`*/`"), |c| {
            c.next() == Some('*') && c.next() == Some('/')
        }) {
            break;
        }

        if parse_block_comment(c) {
            continue;
        }

        if c.expect(Symbol::new_static("comment character"), |c| {
            c.next().is_some()
        }) {
            continue;
        }

        c.stuck(|_| ()); // Already recovered.
        break;
    }

    true
}

fn parse_open_delimiter(c: &mut StrSequence) -> Option<GroupDelimiter> {
    if c.expect(Symbol::new_static("`{`"), |c| c.next() == Some('{')) {
        return Some(GroupDelimiter::Brace);
    }

    if c.expect(Symbol::new_static("`[`"), |c| c.next() == Some('[')) {
        return Some(GroupDelimiter::Bracket);
    }

    if c.expect(Symbol::new_static("`(`"), |c| c.next() == Some('(')) {
        return Some(GroupDelimiter::Paren);
    }

    None
}

fn parse_close_delimiter(c: &mut StrSequence, expected: GroupDelimiter) -> Option<GroupDelimiter> {
    if c.expect_covert(
        expected == GroupDelimiter::Brace,
        Symbol::new_static("`}`"),
        |c| c.next() == Some('}'),
    ) {
        return Some(GroupDelimiter::Brace);
    }

    if c.expect_covert(
        expected == GroupDelimiter::Bracket,
        Symbol::new_static("`]`"),
        |c| c.next() == Some(']'),
    ) {
        return Some(GroupDelimiter::Bracket);
    }

    if c.expect_covert(
        expected == GroupDelimiter::Paren,
        Symbol::new_static("`)`"),
        |c| c.next() == Some(')'),
    ) {
        return Some(GroupDelimiter::Paren);
    }

    if c.expect_covert(
        expected == GroupDelimiter::File,
        Symbol::new_static("end of file"),
        |c| c.next().is_none(),
    ) {
        return Some(GroupDelimiter::File);
    }

    None
}

fn parse_punct_char(c: &mut StrSequence, normal_mode: bool) -> Option<PunctChar> {
    c.expect(Symbol::new_static("punctuation"), |c| {
        c.next().and_then(PunctChar::from_char).filter(|ch| {
            normal_mode || *ch != punct!('.') || c.peek().is_some_and(|c| !c.is_ascii_digit())
        })
    })
}

fn parse_ident(c: &mut StrSequence, start: Span, is_raw: bool) -> Option<TokenIdent> {
    let mut builder = String::new();

    // Match first character
    builder.push(c.expect(Symbol::new_static("identifier"), |c| {
        c.next().filter(|c| c.is_xid_start() || *c == '_')
    })?);

    // Match subsequent characters
    while let Some(ch) = c.expect(Symbol::new_static("identifier"), |c| {
        c.next().filter(|c| c.is_xid_continue())
    }) {
        builder.push(ch);
    }

    Some(TokenIdent {
        span: start.until(c.next_span()),
        text: Symbol::new(builder.as_str()),
        is_raw,
    })
}

fn parse_string_literal(c: &mut StrSequence) -> Option<TokenStringLit> {
    let start = c.next_span();
    let _wp = c.while_parsing(Symbol::new_static("string literal"));

    // Match opening quote
    if !c.expect(Symbol::new_static("`\"`"), |c| c.next() == Some('"')) {
        return None;
    }

    // Match string contents
    let mut builder = String::new();
    loop {
        // Match character escape
        if c.expect(Symbol::new_static("`\\`"), |c| c.next() == Some('\\')) {
            if let Some(esc) = parse_char_escape(c, true) {
                builder.push(esc);
            }

            continue;
        }

        // Match closing quote
        if c.expect(Symbol::new_static("`\"`"), |c| c.next() == Some('"')) {
            break;
        }

        // Match anything but the EOF
        if let Some(char) = c.expect(Symbol::new_static("string character"), |c| c.next()) {
            builder.push(char);
            continue;
        }

        // Otherwise, we got stuck.
        c.stuck(|_| ());
        break;
    }

    Some(TokenStringLit {
        span: start.until(c.next_span()),
        inner: Symbol::new(builder.as_str()),
    })
}

fn parse_char_literal(c: &mut StrSequence) -> Option<TokenCharLit> {
    let start = c.next_span();

    // Match opening quote
    if !c.expect(Symbol::new_static("`'`"), |c| c.next() == Some('\'')) {
        return None;
    }

    let _wp = c.while_parsing(Symbol::new_static("character literal"));

    // Match inner character
    let ch = 'parse: {
        // Match character escape
        if c.expect(Symbol::new_static("`\\`"), |c| c.next() == Some('\\')) {
            break 'parse parse_char_escape(c, false);
        }

        // Match anything but an EOF, a newline, or a closing quote
        if let Some(char) = c.expect(Symbol::new_static("string character"), |c| {
            c.next().filter(|&c| c != '\'' && c != '\n')
        }) {
            break 'parse Some(char);
        }

        c.hint_stuck_if_passes("try escaping the `'` character using `\\'`", |c| {
            c.next() == Some('\'')
        });

        c.hint_stuck_if_passes(
            "newlines must be written using their escape sequence `\\n`",
            |c| c.next() == Some('\n'),
        );

        // Otherwise, we got stuck.
        c.stuck(|c| {
            if c.peek() == Some('\'') {
                let _ = c.next();
            }
        });
        None
    };

    // Match closing quote
    if !c.expect(Symbol::new_static("`'`"), |c| c.next() == Some('\'')) {
        // Otherwise, we got stuck.
        c.stuck(|_| ());

        // (fallthrough)
    }

    Some(TokenCharLit {
        span: start.until(c.next_span()),
        ch: ch.unwrap_or('\0'),
    })
}

fn parse_char_escape(c: &mut StrSequence, allow_multiline: bool) -> Option<char> {
    let esc_start = c.next_span();
    let _wp = c.while_parsing(Symbol::new_static("character escape code"));

    // Match multiline escape
    if allow_multiline {
        if c.expect(Symbol::new_static("newline"), |c| c.next() == Some('\n')) {
            // Match leading whitespace
            while c.expect(Symbol::new_static("escaped space"), |c| {
                c.next().is_some_and(|c| c.is_whitespace())
            }) {}

            // Match pipe
            let _ = c.expect(Symbol::new_static("`|`"), |c| c.next() == Some('|'));

            return None;
        }
    } else {
        c.hint_stuck_if_passes("you can only escape newlines in string literals", |c| {
            c.next() == Some('\n')
        });
    }

    // Match simple escapes
    for (expected, ch, decoded) in [
        // Quotes
        (Symbol::new_static("`\"`"), '"', '"'),
        (Symbol::new_static("`'`"), '\'', '\''),
        // ASCII
        (Symbol::new_static("`n`"), 'n', '\n'),
        (Symbol::new_static("`r`"), 'r', '\r'),
        (Symbol::new_static("`t`"), 't', '\t'),
        (Symbol::new_static("`\\`"), '\\', '\\'),
        (Symbol::new_static("`0`"), '0', '0'),
    ] {
        if c.expect(expected, |c| c.next() == Some(ch)) {
            return Some(decoded);
        }
    }

    // Match ASCII code escapes
    if c.expect(Symbol::new_static("`x`"), |c| c.next() == Some('x')) {
        let _wp = c.while_parsing(Symbol::new_static("ASCII escape code"));
        let hex_start = c.next_span();

        let Some((a, b)) = c.expect(Symbol::new_static("two hexadecimal digits"), |c| {
            match (c.next(), c.next()) {
                (Some(a), Some(b)) if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() => {
                    Some((a, b))
                }
                _ => None,
            }
        }) else {
            c.stuck(|c| {
                while c.peek().is_some_and(|v| !v.is_whitespace() && v != '"') {
                    let _ = c.next();
                }
            });
            return None;
        };

        let hex = u8::from_str_radix(&format!("{a}{b}"), 16).unwrap();
        if hex > 0x7F {
            c.error(
                Diagnostic::span_err(
                    hex_start.until(c.next_span()),
                    "invalid ASCII escape code (must be 0x7F or less)",
                ),
                |_| (),
            );
            return None;
        }

        return Some(hex as char);
    }

    // Match Unicode escapes
    if c.expect(Symbol::new_static("`u`"), |c| c.next() == Some('u')) {
        let _wp = c.while_parsing(Symbol::new_static("Unicode escape sequence"));

        // Match opening brace
        if !c.expect(Symbol::new_static("`{`"), |c| c.next() == Some('{')) {
            c.stuck(|_| ());
            return None;
        }

        // Match digits
        let mut digits = String::new();
        while digits.len() < 6 {
            // Match a hexadecimal digit
            if let Some(digit) = c.expect(Symbol::new_static("hexadecimal digit"), |c| {
                c.next().filter(char::is_ascii_hexdigit)
            }) {
                digits.push(digit);
                continue;
            }

            // Match an underscore
            if c.expect(Symbol::new_static("`_`"), |c| c.next() == Some('_')) {
                continue;
            }

            break;
        }

        // If we have an insufficient number of digits or fail to match a closing `}`, we're stuck.
        if digits.is_empty() || !c.expect(Symbol::new_static("`}`"), |c| c.next() == Some('}')) {
            c.hint_stuck_if_passes("expected at least 1 hexadecimal digit", |c| {
                c.next() == Some('}')
            });

            c.hint_stuck_if_passes("expected at most 6 hexadecimal digits", |c| {
                c.next().is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            });

            c.stuck_lookahead(|c| loop {
                match c.next() {
                    Some('}') => break true,
                    Some('\n') | None => break false,
                    _ => continue,
                }
            });
            return None;
        }

        // Parse hex-code
        let code = u32::from_str_radix(&digits, 16).unwrap();

        // Validate code
        let Some(ch) = char::from_u32(code) else {
            c.error(
                Diagnostic::span_err(
                    esc_start.until(c.next_span()),
                    format!("unicode escape {digits:?} is invalid"),
                ),
                |_| (),
            );
            return None;
        };

        return Some(ch);
    }

    // Otherwise, we're stuck.
    c.stuck(|_| ());

    None
}

fn parse_numeric_literal(c: &mut StrSequence) -> Option<TokenNumberLit> {
    let start = c.next_span();
    let mut builder = String::new();

    // Match first digit
    let digit = c.expect(Symbol::new_static("numeric literal"), |c| {
        c.next().filter(|c| c.is_ascii_digit())
    })?;
    builder.push(digit);

    // Natch prefix
    let prefix = if digit == '0' {
        if c.expect(Symbol::new_static("`x`"), |c| c.next() == Some('x')) {
            builder.push('x');
            DigitKind::Hexadecimal
        } else if c.expect(Symbol::new_static("`b`"), |c| c.next() == Some('b')) {
            builder.push('b');
            DigitKind::Binary
        } else if c.expect(Symbol::new_static("`o`"), |c| c.next() == Some('o')) {
            builder.push('o');
            DigitKind::Octal
        } else {
            DigitKind::Decimal
        }
    } else {
        c.hint_stuck_if_passes(
            "prefixes cannot be used unless the first digit of the number is a `0`",
            |c| matches!(c.next(), Some('x' | 'b' | 'o')),
        );

        DigitKind::Decimal
    };

    // Match integral part
    parse_digits(c, &mut builder, prefix);

    // Match fractional part
    if prefix == DigitKind::Decimal {
        if c.expect(Symbol::new_static("`.`"), |c| c.next() == Some('.')) {
            builder.push('.');

            // Match zero or more fractional digits
            parse_digits(c, &mut builder, DigitKind::Decimal);
        }
    } else {
        c.hint_stuck_if_passes(
            format_args!(
                "fractional parts cannot be specified for {} numbers",
                prefix.prefix()
            ),
            |c| c.next() == Some('.'),
        );
    }

    // Match exponential part
    if prefix == DigitKind::Decimal {
        if c.expect(Symbol::new_static("`e`"), |c| {
            matches!(c.next(), Some('e' | 'E'))
        }) {
            // Match sign
            if c.expect(Symbol::new_static("`+`"), |c| c.next() == Some('+')) {
                builder.push('+');
            } else if c.expect(Symbol::new_static("`-`"), |c| c.next() == Some('-')) {
                builder.push('-');
            };

            // Match zero or more exponent digits
            parse_digits(c, &mut builder, DigitKind::Decimal);
        }
    } else {
        c.hint_stuck_if_passes(
            format_args!(
                "exponent cannot be specified for {} numbers",
                prefix.prefix()
            ),
            |c| matches!(c.next(), Some('e' | 'E')),
        );
    }

    // Match suffix
    let suffixes = [
        "usize", "isize", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
        "f32", "f64",
    ];
    if let Some(suffix) = c.expect(Symbol::new_static("numeric type suffix"), |c| {
        suffixes
            .iter()
            .find(|e| c.lookahead(|c| e.chars().all(|e| c.next() == Some(e))))
    }) {
        builder.push_str(suffix);
    } else {
        c.hint_stuck_if_passes("this is not a valid suffix integer suffix", |c| {
            if c.next().is_some_and(|c| c.is_xid_start()) {
                while c.peek().is_some_and(|c| c.is_xid_continue()) {
                    c.next();
                }
                true
            } else {
                false
            }
        });
    }

    Some(TokenNumberLit {
        span: start.until(c.next_span()),
        data: Symbol::new(builder.as_str()),
    })
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum DigitKind {
    Hexadecimal,
    Binary,
    Octal,
    Decimal,
}

impl DigitKind {
    fn prefix(self) -> &'static str {
        match self {
            DigitKind::Hexadecimal => "hexadecimal",
            DigitKind::Binary => "binary",
            DigitKind::Octal => "octal",
            DigitKind::Decimal => "decimal",
        }
    }
}

fn parse_digits(c: &mut StrSequence, builder: &mut String, kind: DigitKind) {
    loop {
        // Match digit
        if let Some(matched) = match kind {
            DigitKind::Hexadecimal => c.expect(Symbol::new_static("decimal digit"), |c| {
                c.next().filter(|c| c.is_ascii_hexdigit())
            }),
            DigitKind::Binary => c.expect(Symbol::new_static("binary digit"), |c| {
                c.next().filter(|&c| matches!(c, '0'..='1'))
            }),
            DigitKind::Octal => c.expect(Symbol::new_static("octal digit"), |c| {
                c.next().filter(|&c| matches!(c, '0'..='7'))
            }),
            DigitKind::Decimal => c.expect(Symbol::new_static("decimal digit"), |c| {
                c.next().filter(|c| c.is_ascii_digit())
            }),
        } {
            builder.push(matched);
            continue;
        } else {
            c.hint_stuck_if_passes(
                format_args!("this is not a valid digit for a {} number", kind.prefix()),
                |c| c.next().is_some_and(|c| c.is_ascii_hexdigit()),
            );
        }

        // Match underscore
        if c.expect(Symbol::new_static("`_`"), |c| c.next() == Some('_')) {
            continue;
        }

        break;
    }
}

// === TokenCursor === //

pub type TokenSequence<'a> = ParseSequence<'a, TokenCursor<'a>>;

pub trait TokenParser: for<'a> OptionParser<TokenCursor<'a>> {}

impl<O: for<'a> OptionParser<TokenCursor<'a>>> TokenParser for O {}

pub fn make_token_parser<O>(
    expectation: Symbol,
    f: impl Fn(&mut TokenCursor<'_>, &mut ParseHinter) -> O,
) -> impl TokenParser<Output = O>
where
    O: LookaheadResult,
{
    (expectation, f)
}

#[derive(Debug, Clone)]
pub struct TokenCursor<'a> {
    pub prev_span: Span,
    pub close_span: Span,
    pub iter: std::slice::Iter<'a, Token>,
}

impl<'a> Iterator for TokenCursor<'a> {
    type Item = &'a Token;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.iter.next()?;
        self.prev_span = token.span();
        Some(token)
    }
}
impl ForkableCursor for TokenCursor<'_> {}

impl ParseCursor for TokenCursor<'_> {
    fn next_span(&self) -> Span {
        self.peek().map_or(self.close_span, |token| token.span())
    }
}

impl LookBackParseCursor for TokenCursor<'_> {
    fn prev_span(&self) -> Span {
        self.prev_span
    }
}
