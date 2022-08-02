use crucible_core::c_enum::CEnum;
use geode::prelude::*;
use unicode_xid::UnicodeXID;

use crate::util::intern::Interner;

use super::{
	file::{FileAtom, FileReader, LoadedFile, Span},
	generic::{
		Cursor, CursorRecovery, CursorUnstuck, ForkableCursor, PResult, ParseError, ParsingErrorExt,
	},
	token_ast::{
		BoxedToken, GroupDelimiterChar, GroupDelimiterKind, PunctKind, Token, TokenCharLit,
		TokenIdent, TokenPunct,
	},
};

type PResultRecoverable<'a, T, M = ()> = Result<T, CursorRecovery<FileReader<'a>, M>>;

pub struct Tokenizer<'a> {
	sess: Session<'a>,
	unstuck: CursorUnstuck<FileReader<'a>>,
	interner: &'a mut Interner,
}

impl<'a> Tokenizer<'a> {
	pub fn tokenize_file(s: Session, file: &LoadedFile) {
		todo!();
	}

	fn match_delimiter(
		&mut self,
		reader: &mut FileReader<'a>,
	) -> PResult<(GroupDelimiterKind, GroupDelimiterChar)> {
		reader.lookahead(|reader| {
			let read = reader.consume_atom().as_char();

			self.unstuck.expect(reader, "group delimiter");

			GroupDelimiterChar::variants()
				.find_map(|variant| match variant.as_char_or_eof() {
					Some((open, close)) => {
						if Some(open) == read {
							Some((GroupDelimiterKind::Open, variant))
						} else if Some(close) == read {
							Some((GroupDelimiterKind::Close, variant))
						} else {
							None
						}
					}
					None => {
						if read.is_none() {
							Some((GroupDelimiterKind::Close, variant))
						} else {
							None
						}
					}
				})
				.ok_or(ParseError)
		})
	}

	fn match_ident(&mut self, reader: &mut FileReader<'a>) -> PResult<BoxedToken> {
		reader.lookahead(|reader| {
			self.unstuck.expect(reader, "identifier");

			// Match XID start
			let (start_loc, first) = reader.consume();
			let first = first.as_char().unwrap();
			if !first.is_xid_start() {
				return Err(ParseError);
			}

			// Match XID cont
			let mut text = self.interner.begin_intern();
			text.push_char(first);

			reader.lookahead_while(|reader| {
				self.unstuck.expect(reader, "identifier continuation");
				let next = reader.consume_atom();

				let next = match next {
					FileAtom::Codepoint(char) if char.is_xid_continue() => char,
					_ => return false,
				};
				text.push_char(next);

				true
			});

			// Construct token
			let text = text.finish();
			let span = Span::new(start_loc, reader.latest_loc());

			Ok(Token::Ident(TokenIdent { span, text }).box_obj(self.sess))
		})
	}

	fn match_punct(&mut self, reader: &mut FileReader<'a>, glued: bool) -> PResult<BoxedToken> {
		let s = self.sess;

		reader.lookahead(|reader| {
			// Consume atom
			self.unstuck.expect(reader, "punct");
			let (loc, char) = reader.consume();
			let span = loc.span();

			// Parse punct mark
			let char = char.as_codepoint().ok_or(ParseError)?;
			let kind = PunctKind::variants()
				.find(|kind| kind.char() == char)
				.ok_or(ParseError)?;

			// Construct bundle
			Ok(Token::Punct(TokenPunct { span, kind, glued }).box_obj(self.sess))
		})
	}

	fn match_char_lit(
		&mut self,
		reader: &mut FileReader<'a>,
	) -> PResultRecoverable<'a, BoxedToken> {
		let mut recovery = CursorRecovery::new(reader, ());

		reader.lookahead(|reader| {
			self.unstuck.expect(reader, "character literal");

			let start = reader.peek_loc();

			// Match opening delimiter
			self.match_seq(reader, "'").or_recoverable(&recovery)?;

			// Match middle character, recovering on an invalid middle character.
			let char = match self.match_lit_char(reader) {
				Ok(char) => Some(char),
				Err(recovery) => {
					// If we fail, recover to the character matching safe point and try to consume
					// the rest of the atom in a sensible manner.
					recovery.recover(reader);
					None
				}
			};

			// Match closing delimiter
			recovery.propose(reader, ());
			self.match_seq(reader, "'").or_recoverable(&recovery)?;
			recovery.propose(reader, ());

			// If the middle character wasn't invalid.
			let char = char.or_recoverable(&recovery)?;

			// Construct token
			let span = Span::new(start, reader.latest_loc());

			Ok(Token::CharLit(TokenCharLit { span, char }).box_obj(self.sess))
		})
	}

	fn match_lit_char(&mut self, reader: &mut FileReader<'a>) -> PResultRecoverable<'a, char> {
		let mut recovery = CursorRecovery::new(reader, ());

		reader.lookahead(|reader| {
			let char = reader
				.consume_atom()
				.as_codepoint()
				.or_recoverable(&recovery)?;

			if char == '\\' {
				self.unstuck
					.expect_many(reader, ["x", "u", "n", "r", "t", "\\", "0", "'", "\""]);

				let char = reader
					.consume_atom()
					.as_codepoint()
					.or_recoverable(&recovery)?;

				match char {
					// Encoded escapes
					'x' | 'X' => {
						self.unstuck.expect(
							reader,
							"2 character hex ASCII character code less than `0x77`",
						);

						// If we fail to read the hex double, pretend that the user just tried to
						// escape the character 'x' but failed and treat that as the malformed unit.
						recovery.propose(reader, ());

						let first = reader
							.consume_atom()
							.as_codepoint()
							.or_recoverable(&recovery)?;

						// Same as above.
						recovery.propose(reader, ());

						let second = reader
							.consume_atom()
							.as_codepoint()
							.or_recoverable(&recovery)?;

						let hex = [first, second].into_iter().collect::<String>();
						let hex = u8::from_str_radix(hex.as_str(), 16)
							.ok()
							.or_recoverable(&recovery)?;

						if hex <= 0x7F {
							Ok(char::from_u32(hex.into()).unwrap())
						} else {
							// TODO: Amend the previous `expect` to include a hint that the specified
							// hex code is past `0x7F`.
							Err(recovery)
						}
					}
					'u' | 'U' => todo!(),

					// Char escapes
					'n' => Ok('\n'),
					'r' => Ok('\r'),
					't' => Ok('\t'),
					'\\' => Ok('\\'),
					'0' => Ok('\0'),
					'\'' => Ok('\''),
					'"' => Ok('"'),

					_ => {
						// The user tried to escape this character but failed. Let's treat this as a
						// malformed escape and skip it.
						recovery.propose(reader, ());

						Err(recovery)
					}
				}
			} else {
				Ok(char)
			}
		})
	}

	fn match_line_comment(&mut self, reader: &mut FileReader<'a>) -> PResult<()> {
		reader.lookahead(|reader| {
			// Match "//"
			self.unstuck.expect(reader, "//");
			self.match_seq(reader, "//")?;

			// Match until end of line or EOF
			reader.lookahead_while(|reader| {
				!matches!(
					reader.consume_atom(),
					FileAtom::Eof | FileAtom::Newline { .. }
				)
			});

			Ok(())
		})
	}

	fn match_seq(&mut self, reader: &mut FileReader<'a>, seq: &str) -> PResult<()> {
		reader.lookahead(|reader| {
			for char in seq.chars() {
				let next = reader.consume_atom().as_char();

				if next != Some(char) {
					return Err(ParseError);
				}
			}

			Ok(())
		})
	}
}
