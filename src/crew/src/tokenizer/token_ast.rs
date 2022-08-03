use std::sync::Arc;

use super::file::{FileLoc, Span};
use crate::util::intern::Intern;
use crucible_core::c_enum::{c_enum, CEnum};

use smallvec::SmallVec;

// === C enums === //

c_enum! {
	/// A punctuating character. Can be further differentiated by whether it's glued to its predecessor
	/// (e.g. differentiating `+=` from `+ =`)
	pub enum PunctKind {
		Backtick,
		Tilde,
		Exclamation,
		At,
		Pound,
		Dollar,
		Percent,
		Caret,
		Ampersand,
		Asterisk,
		Dash,
		Plus,
		Equals,
		Mid,
		Backslash,
		Semicolon,
		Colon,
		Comma,
		Lt,
		Period,
		Gt,
		Slash,
		Question,
	}

	/// The character used to delimit a token group.
	pub enum GroupDelimiterChar {
		/// A brace (`{}`) delimiter.
		Brace,

		/// A bracket (`[]`) delimiter.
		Bracket,

		/// A parenthetical (`()`) delimiter.
		Paren,

		/// A file delimiter (`SOF ... EOF`)
		File,
	}

	/// Whether a group delimiter is an opening one or a closing one.
	pub enum GroupDelimiterKind {
		Open,
		Close,
	}

}

impl PunctKind {
	pub fn try_from_char(char: char) -> Option<Self> {
		Self::variants().find(|var| var.char() == char)
	}

	pub fn char(self) -> char {
		match self {
			PunctKind::Backtick => '`',
			PunctKind::Tilde => '~',
			PunctKind::Exclamation => '!',
			PunctKind::At => '@',
			PunctKind::Pound => '#',
			PunctKind::Dollar => '$',
			PunctKind::Percent => '%',
			PunctKind::Caret => '^',
			PunctKind::Ampersand => '&',
			PunctKind::Asterisk => '*',
			PunctKind::Dash => '-',
			PunctKind::Plus => '+',
			PunctKind::Equals => '=',
			PunctKind::Mid => '|',
			PunctKind::Backslash => '\\',
			PunctKind::Semicolon => ';',
			PunctKind::Colon => ':',
			PunctKind::Comma => ',',
			PunctKind::Lt => '<',
			PunctKind::Period => '.',
			PunctKind::Gt => '>',
			PunctKind::Slash => '/',
			PunctKind::Question => '?',
		}
	}
}

impl GroupDelimiterChar {
	pub fn as_char_or_eof(&self) -> Option<(char, char)> {
		match self {
			GroupDelimiterChar::Brace => Some(('{', '}')),
			GroupDelimiterChar::Bracket => Some(('[', ']')),
			GroupDelimiterChar::Paren => Some(('(', ')')),
			GroupDelimiterChar::File => None,
		}
	}
}

// === Tree === //

// Token
#[derive(Debug, Clone)]
pub enum Token {
	Group(TokenGroup),
	Ident(TokenIdent),
	Punct(TokenPunct),
	CharLit(TokenCharLit),
	StrLit(TokenStrLit),
	NumberLit(TokenNumberLit),
}

// TokenGroup
#[derive(Debug, Clone)]
pub struct TokenGroup {
	pub span: Span,
	pub delimiter: GroupDelimiterChar,
	pub tokens: Arc<Vec<Token>>,
}

impl TokenGroup {
	pub fn tokens_mut(&mut self) -> &mut Vec<Token> {
		Arc::make_mut(&mut self.tokens)
	}
}

// TokenIdent
#[derive(Debug, Clone)]
pub struct TokenIdent {
	pub span: Span,
	pub text: Intern,
}

// TokenPunct
#[derive(Debug, Clone)]
pub struct TokenPunct {
	pub loc: FileLoc,
	pub kind: PunctKind,
	pub glued: bool,
}

// TokenCharLit
#[derive(Debug, Clone)]
pub struct TokenCharLit {
	pub span: Span,
	pub char: char,
}

// TokenStrLit
#[derive(Debug, Clone)]
pub struct TokenStrLit {
	pub span: Span,
	pub parts: SmallVec<[TokenStrLitPart; 1]>,
}

#[derive(Debug, Clone)]
pub enum TokenStrLitPart {
	Textual(TokenStrLitTextualPart),
	Group(TokenGroup),
}

#[derive(Debug, Clone)]
pub struct TokenStrLitTextualPart {
	span: Span,
	text: Intern,
}

// TokenNumberLit
#[derive(Debug, Clone)]
pub struct TokenNumberLit {
	pub span: Span,
}
