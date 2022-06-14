use crate::util::{intern::Intern, obj::Entity};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum TokenKind {
	Ident,
	Group,
	StringLit,
	NumberLit,
	Comment,
	Formatting,
}

#[derive(Debug, Clone)]
pub struct TokenIdent {}

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
	delimiter: TokenGroupDelimiter,
	tokens: Vec<Entity>,
}

#[derive(Debug, Clone)]
pub struct TokenStringLit {}

#[derive(Debug, Clone)]
pub enum TokenStringLitPart {
	Literal(Intern),
	Group(Entity),
}

#[derive(Debug, Clone)]
pub struct TokenNumberLit {}

#[derive(Debug, Clone)]
pub struct TokenComment {}

#[derive(Debug, Clone)]
pub struct TokenFormatting {}
