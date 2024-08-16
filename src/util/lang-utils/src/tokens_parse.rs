use core::fmt;
use std::hash;

use crate::{
    parser::{LookBackParseCursor as _, ParseCursor as _},
    span::Span,
    symbol::Symbol,
    tokens::{
        make_token_parser, punct, GroupDelimiter, PunctChar, Token, TokenCharLit, TokenCursor,
        TokenGroup, TokenIdent, TokenNumberLit, TokenParser, TokenStringLit,
    },
};

// == Keyword === //

pub trait Keyword: 'static + Sized + Copy + Eq + hash::Hash + fmt::Debug {
    fn from_str(kw: &str) -> Option<Self>;

    fn as_str(self) -> &'static str;

    fn as_name_str(self) -> &'static str;

    fn from_sym(kw: Symbol) -> Option<Self> {
        Self::from_str(kw.as_str())
    }

    fn as_sym(self) -> Symbol {
        Symbol::new_static(self.as_str())
    }

    fn as_name_sym(self) -> Symbol {
        Symbol::new_static(self.as_name_str())
    }
}

#[doc(hidden)]
pub mod define_keywords_internals {
    pub use {
        super::Keyword,
        crucible_utils::hash::FxHashMap,
        std::{concat, fmt, option::Option, primitive::str, thread_local, write},
    };
}

#[macro_export]
macro_rules! define_keywords {
    ($(
        $(#[$attr:meta])*
        $vis:vis enum $enum_name:ident {
            $($name:ident = $text:expr),*
            $(,)?
        }
    )*) => {$(
        $(#[$attr])*
        #[derive(Copy, Clone, Hash, Eq, PartialEq)]
        $vis enum $enum_name {
            $($name),*
        }

        const _: () = {
            use $crate::tokens_parse::define_keywords_internals::*;

            impl Keyword for $enum_name {
                fn from_str(c: &str) -> Option<Self> {
                    thread_local! {
                        static SYM_MAP: FxHashMap<&'static str, $enum_name> = FxHashMap::from_iter([
                            $(($text, $enum_name::$name),)*
                        ]);
                    }

                    SYM_MAP.with(|v| v.get(c).copied())
                }

                fn as_str(self) -> &'static str {
                    const SYM_MAP: [&'static str; 0 $(+ { let _ = $enum_name::$name; 1})*] = [
                        $($text,)*
                    ];

                    SYM_MAP[self as usize]
                }

                fn as_name_str(self) -> &'static str {
                    const SYM_MAP: [&'static str; 0 $(+ { let _ = $enum_name::$name; 1})*] = [
                        $(concat!("`", $text, "`"),)*
                    ];

                    SYM_MAP[self as usize]
                }
            }

            impl fmt::Debug for $enum_name {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{}", self.as_str())
                }
            }
        };
    )*};
}

pub use define_keywords;

// === Parsers === //

pub fn identifier<K: Keyword>(name: Symbol) -> impl TokenParser<Output = Option<TokenIdent>> {
    make_token_parser(name, |c: &mut TokenCursor<'_>, hint| {
        let ident = *c.next()?.as_ident()?;

        if !ident.is_raw && K::from_sym(ident.text).is_some() {
            hint.hint(c.prev_span(), "this identifier has been reserved as a keyword; prefix it with `@` to interpret it as an identifier");
            return None;
        }

        Some(ident)
    })
}

pub fn keyword(kw: impl Keyword) -> impl TokenParser<Output = Option<TokenIdent>> {
    make_token_parser(kw.as_name_sym(), move |c, _| {
        c.next()
            .and_then(Token::as_ident)
            .copied()
            .filter(|ident| !ident.is_raw && ident.text == kw.as_sym())
    })
}

pub fn punct_seq(
    name: Symbol,
    puncts: &[PunctChar],
) -> impl TokenParser<Output = Option<Span>> + '_ {
    make_token_parser(name, move |c, _| {
        let start = c.next_span();
        let mut last = start;

        puncts
            .iter()
            .enumerate()
            .all(|(i, expected)| {
                last = c.next_span();
                let Some(Token::Punct(punct)) = c.next() else {
                    return false;
                };

                if i != 0 && !punct.glued {
                    return false;
                }

                punct.char == *expected
            })
            .then(|| start.to(last))
    })
}

pub fn turbo() -> impl TokenParser<Output = Option<Span>> {
    punct_seq(Symbol::new_static("`::`"), &[punct!(':'), punct!(':')])
}

pub fn wide_arrow() -> impl TokenParser<Output = Option<Span>> {
    punct_seq(Symbol::new_static("`=>`"), &[punct!('='), punct!('>')])
}

pub fn thin_arrow() -> impl TokenParser<Output = Option<Span>> {
    punct_seq(Symbol::new_static("`->`"), &[punct!('-'), punct!('>')])
}

pub fn short_and() -> impl TokenParser<Output = Option<Span>> {
    punct_seq(Symbol::new_static("`&&`"), &[punct!('&'), punct!('&')])
}

pub fn short_or() -> impl TokenParser<Output = Option<Span>> {
    punct_seq(Symbol::new_static("`||`"), &[punct!('|'), punct!('|')])
}

pub fn punct(char: PunctChar) -> impl TokenParser<Output = Option<Span>> {
    make_token_parser(char.as_char_name(), move |c, _| {
        let span = c.next_span();
        c.next()
            .and_then(Token::as_punct)
            .is_some_and(|c| c.char == char)
            .then_some(span)
    })
}

pub fn del_group(delimiter: GroupDelimiter) -> impl TokenParser<Output = Option<TokenGroup>> {
    let expectation = match delimiter {
        GroupDelimiter::Brace => Symbol::new_static("`{`"),
        GroupDelimiter::Bracket => Symbol::new_static("`[`"),
        GroupDelimiter::Paren => Symbol::new_static("`(`"),
        GroupDelimiter::File => unreachable!(),
    };

    make_token_parser(expectation, move |c, _| {
        c.next()
            .and_then(Token::as_group)
            .filter(|g| g.delimiter == delimiter)
            .cloned()
    })
}

pub fn str_lit(name: Symbol) -> impl TokenParser<Output = Option<TokenStringLit>> {
    make_token_parser(name, move |c, _| match c.next() {
        Some(Token::StringLit(lit)) => Some(*lit),
        _ => None,
    })
}

pub fn char_lit(name: Symbol) -> impl TokenParser<Output = Option<TokenCharLit>> {
    make_token_parser(name, |c, _| match c.next() {
        Some(Token::CharLit(lit)) => Some(*lit),
        _ => None,
    })
}

pub fn number_lit(name: Symbol) -> impl TokenParser<Output = Option<TokenNumberLit>> {
    make_token_parser(name, |c, _| match c.next() {
        Some(Token::NumberLit(lit)) => Some(*lit),
        _ => None,
    })
}

pub fn eof(name: Symbol) -> impl TokenParser<Output = bool> {
    make_token_parser(name, |c, _| c.next().is_none())
}
