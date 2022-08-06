use std::{cell::RefCell, mem::replace, rc::Rc, sync::Arc};

use crucible_core::c_enum::CEnum;
use smallvec::SmallVec;
use unicode_xid::UnicodeXID;

use crate::util::intern::{Intern, Interner};

use super::{
	file::{FileAtom, FileLoc, FileReader, LoadedFile, Span},
	generic::{
		Cursor, CursorRecovery, CursorRecoveryExt, CursorUnstuck, ForkableCursor, PResult,
		ParseError, ParsingErrorExt,
	},
	token_ir::{
		GroupDelimiterChar, GroupDelimiterKind, NumLitExpSign, NumLitPrefix, PunctKind, Token,
		TokenCharLit, TokenGroup, TokenIdent, TokenNumLit, TokenPunct, TokenStrLit,
		TokenStrLitPart, TokenStrLitTextualPart,
	},
};

// === Entry === //

pub fn tokenize(interner: &mut Interner, file: &LoadedFile) -> TokenGroup {
	// Acquire all relevant context
	let mut unstuck = CursorUnstuck::new();
	let mut cx = TokenizerCx { interner };
	let mut stack = TokenizerState::new(file);
	let mut reader = file.reader();

	// Run main loop
	loop {
		match stack.top() {
			TokenizerFrame::Group(group) => {
				let mut recovery = CursorRecovery::new_one_token_after(&reader, ());

				enum LookaheadRes {
					Token(Token),
					OpenBlockComment,
					OpenGroup(FileLoc, GroupDelimiterChar),
					CloseGroup(FileLoc, GroupDelimiterChar),
					OpenStrLit(FileLoc),
					Whitespace,
				}

				let result = reader
					.lookahead_cases(Err(ParseError))
					// === General === //
					// Identifiers
					.case(|reader| {
						let can_match_ident = !group.just_matched_num_lit();

						let ident = reader.commit_if(can_match_ident, |reader| {
							let mut null = CursorUnstuck::null();

							cx.match_ident(
								if can_match_ident {
									&mut unstuck
								} else {
									&mut null
								},
								reader,
							)
						});

						if ident.is_ok() && !can_match_ident {
							unstuck.rejection_hint(
								reader,
								"Identifier cannot occur immediately after a numeric literal.",
							);
							return Err(ParseError);
						}

						Ok(LookaheadRes::Token(Token::Ident(ident?)))
					})
					// Character literal
					.case(|reader| {
						let char_lit = cx
							.match_char_lit(&mut unstuck, reader)
							.push_recovery(&mut recovery, |_| ())?;

						Ok(LookaheadRes::Token(Token::CharLit(char_lit)))
					})
					// String literal start
					.case(|reader| {
						let loc = reader.peek_loc();
						cx.match_seq(reader, "\"")?;
						Ok(LookaheadRes::OpenStrLit(loc))
					})
					// Numeric literal
					.case(|reader| {
						let num_lit = cx
							.match_num_lit(&mut unstuck, reader)
							.push_recovery(&mut recovery, |_| ())?;

						Ok(LookaheadRes::Token(Token::NumLit(num_lit)))
					})
					// Line comment
					.case(|reader| {
						cx.match_line_comment(&mut unstuck, reader)?;
						Ok(LookaheadRes::Whitespace)
					})
					// Whitespace (you have been living here for as long as you can remember)
					.case(|reader| match reader.consume_atom() {
						FileAtom::Codepoint(point) if point.is_whitespace() => {
							Ok(LookaheadRes::Whitespace)
						}
						FileAtom::Newline { .. } => Ok(LookaheadRes::Whitespace),
						_ => Err(ParseError),
					})
					// Group delimiters
					.case(|reader| {
						let loc = reader.peek_loc();
						let (kind, delimiter) = cx.match_group_delimiter(&mut unstuck, reader)?;

						match kind {
							GroupDelimiterKind::Open => Ok(LookaheadRes::OpenGroup(loc, delimiter)),
							GroupDelimiterKind::Close => {
								Ok(LookaheadRes::CloseGroup(loc, delimiter))
							}
						}
					})
					// === Punct-started sequences === //
					// Block comment start
					.case(|reader| {
						cx.match_seq(reader, "/*")?;
						stack.open_comment_frame();
						Ok(LookaheadRes::OpenBlockComment)
					})
					.barrier()
					// === Punctuation === //
					.case(|reader| {
						let punct =
							cx.match_punct(&mut unstuck, reader, group.just_matched_punct())?;
						Ok(LookaheadRes::Token(Token::Punct(punct)))
					})
					.finish();

				match result {
					Ok(LookaheadRes::Token(token)) => {
						group.push_token(token);
					}
					Ok(LookaheadRes::OpenBlockComment) => {
						stack.open_comment_frame();
					}
					Ok(LookaheadRes::OpenGroup(loc, delimiter)) => {
						stack.open_group_frame(loc, delimiter);
					}
					Ok(LookaheadRes::CloseGroup(loc, delimiter)) => {
						assert_eq!(group.delimiter(), delimiter); // TODO: Non-panicking recovery
						if let Some(finished_root) = stack.close_frame(&mut cx, loc) {
							return finished_root;
						}
					}
					Ok(LookaheadRes::OpenStrLit(loc)) => {
						stack.open_str_lit_frame(loc);
					}
					Ok(LookaheadRes::Whitespace) => {
						group.notify_whitespace();
					}
					Err(_) => {
						println!("Parse error! {:?}", unstuck.unstuck_hint());
						recovery.recover(&mut reader);
					}
				}
			}
			TokenizerFrame::StrLit(str_lit) => {
				let mut recovery = CursorRecovery::new(&reader, ());

				enum LookaheadRes {
					Char(char),
					TemplatedVar(FileLoc),
					End(FileLoc),
				}

				let result = reader
					.lookahead_cases(Err(ParseError))
					.case(|reader| {
						let char = cx
							.match_str_char(&mut unstuck, reader)
							.push_recovery(&mut recovery, |_| ())?;

						Ok(LookaheadRes::Char(char))
					})
					.case(|reader| {
						cx.match_seq(reader, "${")?;
						Ok(LookaheadRes::TemplatedVar(reader.latest_loc()))
					})
					.case(|reader| {
						cx.match_seq(reader, "\"")?;
						Ok(LookaheadRes::End(reader.latest_loc()))
					})
					.finish();
			}
			TokenizerFrame::BlockComment => {
				if cx.match_seq(&mut reader, "*/").is_ok() {
					stack.close_frame(&mut cx, reader.peek_loc());
				} else {
				}
			}
		}
	}
}

// === `TokenizerContext` === //

type PResultRecoverable<'a, T, M = ()> = Result<T, CursorRecovery<FileReader<'a>, M>>;

struct TokenizerCx<'a> {
	interner: &'a mut Interner,
}

impl<'a> TokenizerCx<'a> {
	fn match_group_delimiter(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResult<(GroupDelimiterKind, GroupDelimiterChar)> {
		reader.lookahead(|reader| {
			unstuck.expect(reader, "group delimiter");

			let read = reader.consume_atom();
			let read_char = read.as_char();

			GroupDelimiterChar::variants()
				.find_map(|variant| match variant.as_char_or_eof() {
					Some((open, close)) => {
						if read_char == open {
							Some((GroupDelimiterKind::Open, variant))
						} else if read_char == close {
							Some((GroupDelimiterKind::Close, variant))
						} else {
							None
						}
					}
					None => {
						if read.is_eof() {
							Some((GroupDelimiterKind::Close, variant))
						} else {
							None
						}
					}
				})
				.ok_or(ParseError)
		})
	}

	fn match_ident(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResult<TokenIdent> {
		reader.lookahead(|reader| {
			unstuck.expect(reader, "identifier");

			// Match XID start
			let (start_loc, first) = reader.consume();
			let first = first.as_char();
			if !first.is_xid_start() && first != '_' {
				return Err(ParseError);
			}

			// Match XID cont
			let mut text = self.interner.begin_intern();
			text.push_char(first);

			reader.lookahead_while(|reader| {
				unstuck.expect(reader, "identifier continuation");
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

	fn match_punct(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
		glued: bool,
	) -> PResult<TokenPunct> {
		reader.lookahead(|reader| {
			// Consume atom
			unstuck.expect(reader, "punct");
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
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResultRecoverable<'a, TokenCharLit> {
		let mut recovery = CursorRecovery::new(reader, ());

		reader.lookahead(|reader| {
			unstuck.expect(reader, "character literal");

			let start = reader.peek_loc();

			// Match opening delimiter
			self.match_seq(reader, "'").or_recoverable(&recovery)?;

			// Match middle character, recovering on an invalid middle character.
			let char = match self.match_str_char(unstuck, reader) {
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

			// TODO: Maybe we could try matching this as a regular string to handle the case where
			// the user thinks `'` and `"` are interchangeable?
			self.match_seq(reader, "'").or_recoverable(&recovery)?;
			recovery.propose(reader, ());

			// If the middle character wasn't invalid.
			let char = char.or_recoverable(&recovery)?;

			// Construct token
			let span = Span::new(start, reader.latest_loc());

			Ok(TokenCharLit { span, char })
		})
	}

	fn match_num_lit(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResultRecoverable<'a, TokenNumLit> {
		reader.lookahead(|reader| {
			let start = reader.peek_loc();

			unstuck.expect(reader, "numeric literal");

			// See if we have a numeric prefix at this position
			let prefix = reader
				.lookahead(|reader| {
					// Match leading `0`
					self.match_seq(reader, "0").ok()?;

					// Match prefix
					unstuck.expect(reader, "numeric literal prefix");
					let char = reader.consume_atom().as_char();

					NumLitPrefix::variants().find(|prefix| prefix.prefix_char() == Some(char))
				})
				.unwrap_or(NumLitPrefix::ImplicitDecimal);

			// Match main non-FP part
			let main_part = self.match_digits(unstuck, reader, prefix.digits());

			// Terminate if the user didn't specify any digits in the base.
			if self.interner.decode(main_part).is_empty() {
				if prefix != NumLitPrefix::ImplicitDecimal {
					// Our recovery is a bit wacky. Essentially, we match *anything* that could
					// possibly be interpreted as part of the number so long as we actually think
					// the user was trying to type a number.
					self.recover_num_lit(reader);
				}

				return Err(CursorRecovery::new(reader, ()));
			}

			// Match digits right of decimal point if relevant to this numeric prefix.
			let float_part = {
				let mut unstuck_at_cursor = reader.clone();
				let can_have_decimal = prefix == NumLitPrefix::ImplicitDecimal;

				// Match decimal point
				if can_have_decimal {
					unstuck.expect(&unstuck_at_cursor, ".");
				}

				let has_dp = reader
					.lookahead_commit_if(can_have_decimal, |reader| {
						// Match point
						self.match_seq(reader, ".")?;

						// Ensure that the decimal is not immediately followed by an identifier start
						// character or a second `.`
						let subsequent = reader.peek_atom().as_char();
						if subsequent.is_xid_start() || subsequent == '_' || subsequent == '.' {
							return Err(ParseError);
						}

						Ok(())
					})
					.is_ok();

				match (has_dp, can_have_decimal) {
					(true, true) => Some(self.match_digits(
						unstuck,
						reader,
						NumLitPrefix::ImplicitDecimal.digits(),
					)),
					(true, false) => {
						unstuck.rejection_hint(
							&unstuck_at_cursor,
							format_args!(
								"Numbers with {} prefix cannot have a decimal point.",
								prefix.with_a_name()
							),
						);
						None
					}
					(false, _) => None,
				}
			};

			// Match exponent
			let exp_part = {
				let mut unstuck_builder = unstuck.bump(reader);
				let can_have_exp = prefix == NumLitPrefix::ImplicitDecimal;

				// Check if we have an exponent.
				if can_have_exp {
					unstuck_builder.expect("e");
				}

				let has_exp = reader.lookahead_commit_if(can_have_exp, |reader| {
					let atom = reader.consume_atom().as_char();
					atom == 'e' || atom == 'E'
				});

				// If we have an exponent
				match (has_exp, can_have_exp) {
					(true, true) => {
						// Look for the optional sign
						let sign = reader
							.lookahead(|reader| {
								unstuck.expect_many(reader, ["+", "-"]);
								match reader.consume_atom().as_char() {
									'+' => Some(NumLitExpSign::Pos),
									'-' => Some(NumLitExpSign::Neg),
									_ => None,
								}
							})
							.unwrap_or(NumLitExpSign::Pos);

						// Read the exponent digits
						let exp_digits = self.match_digits(
							unstuck,
							reader,
							NumLitPrefix::ImplicitDecimal.digits(),
						);

						// Report a grammatical error if there are no exponent digits
						if self.interner.decode(exp_digits).is_empty() {
							self.recover_num_lit(reader);
							return Err(CursorRecovery::new(reader, ()));
						}

						Some((sign, exp_digits))
					}
					(true, false) => {
						unstuck_builder.rejection_hint(format_args!(
							"Numbers with {} prefix cannot have a floating-point exponent.",
							prefix.with_a_name()
						));
						None
					}
					(false, _) => None,
				}
			};

			// Build token
			Ok(TokenNumLit {
				span: Span::new(start, reader.latest_loc()),
				prefix,
				main_part,
				float_part,
				exp_part,
			})
		})
	}

	fn match_digits(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
		set: &str,
	) -> Intern {
		debug_assert!(set
			.chars()
			.all(|char| !char.is_alphabetic() || char.is_ascii_lowercase()));

		let mut intern = self.interner.begin_intern();

		// Match digits
		reader.lookahead_while(|reader| {
			// Emit expectations if appropriate
			unstuck.expect(reader, format_args!("digits ({})", set));

			// Match digit
			let digit = reader.consume_atom().as_char();

			if digit == '_' {
				// (ignore visual separators)
				true
			} else if set.contains(digit.to_ascii_lowercase()) {
				intern.push_char(digit);
				true
			} else {
				false
			}
		});

		// Produce intern
		intern.finish()
	}

	fn recover_num_lit(&mut self, reader: &mut FileReader<'a>) {
		reader.lookahead_while(|reader| {
			let atom = reader.consume_atom().as_char();

			atom.is_xid_continue() || atom == '.' || atom == '+' || atom == '-'
		});
	}

	fn match_str_char(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResultRecoverable<'a, char> {
		let mut recovery = CursorRecovery::new(reader, ());

		reader.lookahead(|reader| {
			let char = reader
				.consume_atom()
				.as_codepoint()
				.or_recoverable(&recovery)?;

			if char == '\\' {
				unstuck.expect_many(reader, ["x", "u", "n", "r", "t", "\\", "0", "'", "\""]);

				let char = reader
					.consume_atom()
					.as_codepoint()
					.or_recoverable(&recovery)?;

				match char {
					// Encoded escapes
					'x' | 'X' => {
						let mut unstuck_builder = unstuck.bump(reader);
						unstuck_builder
							.expect("2 character hex ASCII character code less than `0x77`");

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
							unstuck_builder.rejection_hint(format_args!(
								"Hex code {}{} is above 0x7F—the maximum allowable ASCII character code.",
								first, second
							));
							Err(recovery)
						}
					}
					'u' | 'U' => todo!("Unicode parsing isn't implemented."),

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

	fn match_line_comment(
		&mut self,
		unstuck: &mut CursorUnstuck<FileReader<'a>>,
		reader: &mut FileReader<'a>,
	) -> PResult<()> {
		reader.lookahead(|reader| {
			// Match "//"
			unstuck.expect(reader, "//");
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

				if next != char {
					return Err(ParseError);
				}
			}

			Ok(())
		})
	}
}

// === `TokenizerState` === //

#[derive(Debug)]
struct TokenizerState {
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
				TokenizerFrameInternal::Group(GroupFrameBuilder::new(GroupDelimiterChar::File)),
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
		debug_assert_eq!(self.block_comments, 0);

		self.frames.push((
			start,
			TokenizerFrameInternal::Group(GroupFrameBuilder::new(delimiter)),
		));
	}

	pub fn open_str_lit_frame(&mut self, start: FileLoc) {
		debug_assert!(matches!(self.top(), TokenizerFrame::Group(_)));

		self.frames.push((
			start,
			TokenizerFrameInternal::StrLit(StrLitFrameBuilder(Default::default())),
		))
	}

	pub fn open_comment_frame(&mut self) {
		// If they managed to nest more than `usize::MAX` comments, the user somehow made us
		// process a file with at least `usize::MAX` characters, which would be impressive.
		self.block_comments += 1;
	}

	pub fn close_frame(&mut self, cx: &mut TokenizerCx, end: FileLoc) -> Option<TokenGroup> {
		if self.block_comments > 0 {
			self.block_comments -= 1;
			None
		} else {
			// Pop and close frame.
			let frame = self.frames.pop().unwrap();

			let token = match frame {
				(start, TokenizerFrameInternal::Group(group)) => {
					let mut group = group.0.borrow_mut();

					Token::Group(TokenGroup {
						span: Span::new(start, end),
						delimiter: group.delimiter,
						tokens: Arc::new(replace(&mut group.tokens, Vec::new())),
					})
				}
				(start, TokenizerFrameInternal::StrLit(str_lit)) => Token::StrLit(TokenStrLit {
					span: Span::new(start, end),
					parts: str_lit.0.borrow_mut().finish(cx),
				}),
			};

			// Update frame
			if let Some((_, top)) = self.frames.last() {
				match top {
					TokenizerFrameInternal::Group(group) => {
						group.push_token(token);
					}
					TokenizerFrameInternal::StrLit(str_lit) => str_lit.push_token_group(
						cx,
						match token {
							Token::Group(group) => group,
							_ => unreachable!(),
						},
					),
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
enum TokenizerFrame {
	Group(GroupFrameBuilder),
	StrLit(StrLitFrameBuilder),
	BlockComment,
}

#[derive(Debug, Clone)]
struct GroupFrameBuilder(Rc<RefCell<GroupFrameBuilderInner>>);

#[derive(Debug)]
struct GroupFrameBuilderInner {
	delimiter: GroupDelimiterChar,
	just_pushed_num_lit: bool,
	tokens: Vec<Token>,
}

impl GroupFrameBuilder {
	fn new(delimiter: GroupDelimiterChar) -> Self {
		Self(Rc::new(RefCell::new(GroupFrameBuilderInner {
			delimiter,
			just_pushed_num_lit: false,
			tokens: Vec::new(),
		})))
	}

	pub fn delimiter(&self) -> GroupDelimiterChar {
		self.0.borrow().delimiter
	}

	pub fn push_token(&self, token: Token) {
		let mut inner = self.0.borrow_mut();

		if matches!(&token, Token::NumLit(_)) {
			inner.just_pushed_num_lit = true;
		} else {
			inner.just_pushed_num_lit = false;
		}

		inner.tokens.push(token);
	}

	pub fn notify_whitespace(&self) {
		self.0.borrow_mut().just_pushed_num_lit = false;
	}

	pub fn just_matched_punct(&self) -> bool {
		matches!(self.0.borrow().tokens.last(), Some(Token::Punct(_)))
	}

	pub fn just_matched_num_lit(&self) -> bool {
		self.0.borrow().just_pushed_num_lit
	}
}

#[derive(Debug, Clone)]
struct StrLitFrameBuilder(Rc<RefCell<StrLitFrameBuilderInner>>);

#[derive(Debug, Default)]
struct StrLitFrameBuilderInner {
	parts: SmallVec<[TokenStrLitPart; 1]>,
	textual_span: Option<Span>,
	textual_str_buf: String, // TODO: Automate scratch buffer creation in the `Interner`.
}

impl StrLitFrameBuilder {
	pub fn push_token_group(&self, cx: &mut TokenizerCx, group: TokenGroup) {
		self.0.borrow_mut().push_token_group(cx, group);
	}

	pub fn push_char(&self, loc: FileLoc, char: char) {
		self.0.borrow_mut().push_char(loc, char);
	}
}

impl StrLitFrameBuilderInner {
	fn push_token_group(&mut self, cx: &mut TokenizerCx, group: TokenGroup) {
		self.flush_textual(cx);
	}

	fn push_char(&mut self, loc: FileLoc, char: char) {
		// Increase span
		if self.textual_str_buf.is_empty() {
			self.textual_span = Some(loc.span());
		}
		self.textual_span.as_mut().unwrap().set_end_loc(loc);

		// Push character
		self.textual_str_buf.push(char);
	}

	fn flush_textual(&mut self, cx: &mut TokenizerCx) {
		if !self.textual_str_buf.is_empty() {
			self.parts
				.push(TokenStrLitPart::Textual(TokenStrLitTextualPart {
					span: self.textual_span.unwrap(),
					text: cx.interner.intern_str(self.textual_str_buf.as_str()),
				}));

			self.textual_str_buf.clear();
		}
	}

	fn finish(&mut self, cx: &mut TokenizerCx) -> SmallVec<[TokenStrLitPart; 1]> {
		self.flush_textual(cx);
		replace(&mut self.parts, SmallVec::new())
	}
}
