use std::{cell::RefCell, rc::Rc};

use crucible_core::c_enum::CEnum;
use smallvec::SmallVec;
use unicode_xid::UnicodeXID;

use crate::util::intern::Interner;

use super::{
	file::{FileAtom, FileLoc, FileReader, LoadedFile, Span},
	generic::{
		Cursor, CursorRecovery, CursorUnstuck, ForkableCursor, PResult, ParseError, ParsingErrorExt,
	},
	token_ast::{
		GroupDelimiterChar, GroupDelimiterKind, PunctKind, Token, TokenCharLit, TokenGroup,
		TokenIdent, TokenPunct, TokenStrLit, TokenStrLitPart,
	},
};

// === Entry === //

pub fn tokenize(interner: &mut Interner, file: &LoadedFile) -> TokenGroup {
	// Acquire all relevant context
	let mut cx = TokenizerCx {
		interner,
		unstuck: CursorUnstuck::default(),
	};

	let mut state = TokenizerState::new(file);
	let mut reader = file.reader();

	// Run main loop
	loop {
		match state.top() {
			TokenizerFrame::Group(group) => {
				let mut recovery = CursorRecovery::new(&reader, ());

				let result = reader
					.lookahead_cases(Err(ParseError))
					// Group delimiters
					.case(|reader| {
						let loc = reader.peek_loc();
						let (kind, delimiter) = cx.match_group_delimiter(reader)?;

						match kind {
							GroupDelimiterKind::Open => {
								state.open_group_frame(loc, delimiter);
								Ok(())
							}
							GroupDelimiterKind::Close => {
								todo!()
							}
						}
					})
					//
					.finish();
			}
			TokenizerFrame::StrLit(str_lit) => todo!(),
			TokenizerFrame::BlockComment => todo!(),
		}
	}
}

// === `TokenizerContext` === //

type PResultRecoverable<'a, T, M = ()> = Result<T, CursorRecovery<FileReader<'a>, M>>;

struct TokenizerCx<'a> {
	unstuck: CursorUnstuck<FileReader<'a>>,
	interner: &'a mut Interner,
}

impl<'a> TokenizerCx<'a> {
	fn match_group_delimiter(
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

	fn match_ident(&mut self, reader: &mut FileReader<'a>) -> PResult<TokenIdent> {
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

			Ok(TokenIdent { span, text })
		})
	}

	fn match_punct(&mut self, reader: &mut FileReader<'a>, glued: bool) -> PResult<TokenPunct> {
		reader.lookahead(|reader| {
			// Consume atom
			self.unstuck.expect(reader, "punct");
			let (loc, char) = reader.consume();

			// Parse punct mark
			let char = char.as_codepoint().ok_or(ParseError)?;
			let kind = PunctKind::variants()
				.find(|kind| kind.char() == char)
				.ok_or(ParseError)?;

			// Construct bundle
			Ok(TokenPunct { loc, kind, glued })
		})
	}

	fn match_char_lit(
		&mut self,
		reader: &mut FileReader<'a>,
	) -> PResultRecoverable<'a, TokenCharLit> {
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

			Ok(TokenCharLit { span, char })
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

						// Hey, sometimes people really want to escape `\0xF`. Not sure why and
						// diagnostics will hopefully bring them to their senses but until then,
						// it's the user's prerogative.
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
					'u' | 'U' => todo!("Unicode parsing isn't finished."),

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

// === `TokenizerState` === //

mod tokenizer_state {
	use std::sync::Arc;

	use crate::tokenizer::token_ast::TokenStrLitTextualPart;

	use super::*;

	// === TokenizerState === //

	#[derive(Debug)]
	pub struct TokenizerState {
		frames: Vec<(FileLoc, TokenizerFrameInternal)>,
		block_comments: usize,
	}

	#[derive(Debug, Clone)]
	enum TokenizerFrameInternal {
		Group(GroupFrameBuilder),
		StrLit(StrLitFrameBuilder),
	}

	impl TokenizerState {
		pub fn new(file: &LoadedFile) -> Self {
			Self {
				frames: vec![(
					file.sof_loc(),
					TokenizerFrameInternal::Group(GroupFrameBuilder {
						delimiter: GroupDelimiterChar::File,
						tokens: Default::default(),
					}),
				)],
				block_comments: 0,
			}
		}

		// === Frame management === //

		pub fn top(&self) -> TokenizerFrame {
			if self.block_comments > 0 {
				TokenizerFrame::BlockComment
			} else {
				let last = self.frames.last().unwrap().1.clone();

				match last {
					TokenizerFrameInternal::Group(group) => TokenizerFrame::Group(group),
					TokenizerFrameInternal::StrLit(str_lit) => TokenizerFrame::StrLit(str_lit),
				}
			}
		}

		pub fn open_group_frame(&mut self, start: FileLoc, delimiter: GroupDelimiterChar) {
			debug_assert_ne!(self.block_comments, 0);

			self.frames.push((
				start,
				TokenizerFrameInternal::Group(GroupFrameBuilder {
					delimiter,
					tokens: Default::default(),
				}),
			));
		}

		pub fn open_str_lit_frame(&mut self, start: FileLoc) {
			debug_assert!(matches!(self.top(), TokenizerFrame::Group(_)));

			self.frames.push((
				start,
				TokenizerFrameInternal::StrLit(StrLitFrameBuilder {
					parts: Default::default(),
				}),
			))
		}

		pub fn open_comment_frame(&mut self) {
			// If they managed to nest more than `usize::MAX` comments, the user somehow made us
			// process a file with at least `usize::MAX` characters, which would be impressive.
			self.block_comments += 1;
		}

		pub fn close_frame(&mut self, end: FileLoc) -> Option<TokenGroup> {
			if self.block_comments > 0 {
				self.block_comments -= 1;
				None
			} else {
				// Pop and close frame.
				let frame = self.frames.pop().unwrap();

				let token = match frame {
					(start, TokenizerFrameInternal::Group(group)) => Token::Group(TokenGroup {
						span: Span::new(start, end),
						delimiter: group.delimiter,
						tokens: Arc::new(group.tokens.replace(Vec::new())),
					}),
					(start, TokenizerFrameInternal::StrLit(str_lit)) => {
						Token::StrLit(TokenStrLit {
							span: Span::new(start, end),
							parts: str_lit.parts.replace(SmallVec::new()),
						})
					}
				};

				// Update frame
				if let Some((_, top)) = self.frames.last() {
					match top {
						TokenizerFrameInternal::Group(group) => {
							group.push_token(token);
						}
						TokenizerFrameInternal::StrLit(str_lit) => {
							str_lit.push_token_group(match token {
								Token::Group(group) => group,
								_ => unreachable!(),
							})
						}
					};
					None
				} else {
					Some(match token {
						Token::Group(group) => group,
						_ => unreachable!("the top-level frame must be a group"),
					})
				}
			}
		}
	}

	// === TokenizerFrame === //

	#[derive(Debug, Clone)]
	pub enum TokenizerFrame {
		Group(GroupFrameBuilder),
		StrLit(StrLitFrameBuilder),
		BlockComment,
	}

	#[derive(Debug, Clone)]
	pub struct GroupFrameBuilder {
		delimiter: GroupDelimiterChar,
		tokens: Rc<RefCell<Vec<Token>>>,
	}

	impl GroupFrameBuilder {
		pub fn delimiter(&self) -> GroupDelimiterChar {
			self.delimiter
		}

		pub fn push_token(&self, token: Token) {
			self.tokens.borrow_mut().push(token);
		}
	}

	#[derive(Debug, Clone)]
	pub struct StrLitFrameBuilder {
		parts: Rc<RefCell<SmallVec<[TokenStrLitPart; 1]>>>,
	}

	impl StrLitFrameBuilder {
		pub fn push_token_group(&self, group: TokenGroup) {
			self.parts.borrow_mut().push(TokenStrLitPart::Group(group));
		}

		pub fn push_textual(&self, lit: TokenStrLitTextualPart) {
			self.parts.borrow_mut().push(TokenStrLitPart::Textual(lit));
		}
	}
}

use tokenizer_state::*;
