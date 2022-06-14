use super::file::Span;
use crate::util::{intern::Intern, obj::Entity};
use smallvec::SmallVec;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum TokenKind {
	Ident,
	Group,
	StringLit,
	NumberLit,
	Comment,
}

#[derive(Debug, Clone)]
pub struct TokenIdent {
	pub text: Intern,
}

#[derive(Debug, Clone)]
pub enum TokenGroupDelimiter {
	/// A brace (`{}`) delimiter.
	Brace,

	/// A bracket (`[]`) delimiter.
	Bracket,

	/// A parenthetical (`()`) delimiter.
	Paren,

	/// A file delimiter (`SOF ... EOF`)
	File,

	/// A macro expansion. Used to ensure intuitive associativity for macro expansions.
	///
	/// For example, if `1 * my_macro!()` expanded to `1 * 3 + 5`, we would want it to be interpreted
	/// as `1 * (3 + 5)` so both the macro consumer and producer benefit from intuitive behavior.
	///
	/// These are stringified as `#> <#`. If the tokenizer is set in source parsing mode, typing these
	/// will cause an error.
	Expansion,
}

#[derive(Debug, Clone)]
pub struct TokenGroup {
	pub delimiter: TokenGroupDelimiter,
	pub tokens: Vec<Entity>,
}

#[derive(Debug, Clone)]
pub struct TokenStringLit {
	pub parts: SmallVec<[TokenStringLitPart; 1]>,
}

#[derive(Debug, Clone)]
pub enum TokenStringLitPart {
	Literal { text: Intern, span: Span },
	Group(Entity),
}

#[derive(Debug, Clone)]
pub struct TokenNumberLit {}

#[derive(Debug, Clone)]
pub struct TokenComment {
	pub kind: CommentKind,
	pub text: Intern,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CommentKind {
	DocsLine,
	DocsBlock,
	RegularLine,
	RegularBlock,
	Region,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TokenPunct {
	pub kind: TokenPunctKind,
	pub glued: bool,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum TokenPunctKind {
	Backtick,
	Tilde,
	Exclamation,
	At,
	Pound,
	Dollar,
	Percent,
	Caret,
	Ampersand,
	Dash,
	Plus,
	Equals,
	Bar,
	Semicolon,
	Colon,
	Comma,
	Period,
	Question,
	Slash,
	Less,
	Greater,
}
