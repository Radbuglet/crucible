use std::{cmp::Ordering, ops::DerefMut};

use crucible_core::{
	lifetime::try_transform_or_err,
	std_traits::{OptionLike, ResultLike},
};
use sealed::sealed;
use thiserror::Error;

// === Generic Cursor === //

/// A cursor is a view into a stream of elements (e.g. characters, tokens, etc) which we call *atoms*.
///
/// The cursor stream logically begins and ends with a delimiter. [Cursor::consume] shouldn't
/// emit this starting delimiter atom but [Cursor::latest] sure can if we haven't consumed anything
/// by the time it gets called. The starting and end delimiter may be represented by the same atom.
/// The ending delimiter, what we call an EOF, is said to be saturating *i.e.* once an EOF is returned,
/// the cursor should not return anything other than that EOF atom.
///
/// Smart mutable pointers to [Cursor] instances (*i.e.* objects that [DerefMut] to it) also
/// implement [Cursor] (although they probably won't implement [ForkableCursor]).
pub trait Cursor: Sized {
	/// The location of an atom yielded by the cursor.
	type Loc: Clone + Eq + Ord;

	/// An atom yielded by the cursor. Delimiters (start and end of stream) are treated as atoms
	/// onto their own.
	type Atom: Atom;

	/// Returns the most recently consumed location-atom pair. If nothing has been read yet, return
	/// the the left-hand delimiter atom (this might be represented as an EOF). If the EOF is consumed
	/// several times, calls to `latest` should return the EOF.
	fn latest(&self) -> (Self::Loc, Self::Atom);

	/// Returns the location of the most recently consumed atom.
	///
	/// See [Cursor::latest] for details on semantics.
	fn latest_loc(&self) -> Self::Loc {
		self.latest().0
	}

	/// Returns the data of the most recently consumed atom.
	///
	/// See [Cursor::latest] for details on semantics.
	fn latest_atom(&self) -> Self::Atom {
		self.latest().1
	}

	/// Consumes the next atom on the cursor head, replacing the state of [Cursor::latest] to
	/// reflect what it just returned (yes, this includes EOFs). Should not return the left delimiter
	/// of the stream. EOFs should be saturating *i.e.* after an EOF has been returned, the cursor
	/// should not return anything other than an EOF.
	fn consume(&mut self) -> (Self::Loc, Self::Atom);

	/// Consumes the next atom on the cursor head and returns its location.
	///
	/// See [Cursor::consume] for details on semantics.
	fn consume_loc(&mut self) -> Self::Loc {
		self.consume().0
	}

	/// Consumes the next atom on the cursor head and returns its data.
	///
	/// See [Cursor::consume] for details on semantics.
	fn consume_atom(&mut self) -> Self::Atom {
		self.consume().1
	}
}

impl<P: DerefMut<Target = T>, T: Cursor> Cursor for P {
	type Loc = T::Loc;
	type Atom = T::Atom;

	fn latest(&self) -> (Self::Loc, Self::Atom) {
		(&**self).latest()
	}

	fn latest_loc(&self) -> Self::Loc {
		(&**self).latest_loc()
	}

	fn latest_atom(&self) -> Self::Atom {
		(&**self).latest_atom()
	}

	fn consume(&mut self) -> (Self::Loc, Self::Atom) {
		(&mut **self).consume()
	}

	fn consume_loc(&mut self) -> Self::Loc {
		(&mut **self).consume_loc()
	}

	fn consume_atom(&mut self) -> Self::Atom {
		(&mut **self).consume_atom()
	}
}

/// An extension trait to manually implement on [Cursor] + [Clone] instances, which adds utility
/// methods to consume the cursor contents with lookahead.
pub trait ForkableCursor: Cursor + Clone {
	/// Return the next location-atom pair to be returned by the cursor without modifying the cursor
	/// state.
	fn peek(&self) -> (Self::Loc, Self::Atom) {
		self.clone().consume()
	}

	/// Return the location of the next atom to be returned by the cursor without modifying the cursor
	/// state.
	fn peek_loc(&self) -> Self::Loc {
		self.clone().consume_loc()
	}

	/// Return the data of the next atom to be returned by the cursor without modifying the cursor
	/// state.
	fn peek_atom(&self) -> Self::Atom {
		self.clone().consume_atom()
	}

	/// Applies a match closure to a fork of the cursor, only committing the fork state when
	/// [LookaheadResult::should_commit] returns true.
	fn lookahead<F, R>(&mut self, f: F) -> R
	where
		F: FnOnce(&mut Self) -> R,
		R: LookaheadResult,
	{
		let mut fork = self.clone();
		let res = f(&mut fork);
		if res.should_commit() {
			*self = fork;
		}
		res
	}

	/// Creates an iterator that continuously consumes atoms with `f`, breaking when `f` fails to
	/// match.
	fn lookahead_while<T, F>(&mut self, mut f: F)
	where
		F: FnMut(&mut Self) -> T,
		T: LookaheadResult,
	{
		loop {
			let mut fork = self.clone();
			let res = f(&mut fork);
			if !res.should_commit() {
				break;
			}
			*self = fork;
		}
	}

	/// Constructs a [BranchMatcher] builder object which allows the user to match against several
	/// grammars.
	fn lookahead_cases<R>(&mut self, result: R) -> BranchMatcher<Self, R> {
		BranchMatcher {
			target: self,
			matched_cursor: None,
			result,
			barrier_has_val: false,
		}
	}

	/// Commits the closure's changes to the cursor *iff* `can_commit` is `true`.
	fn commit_if<T, F>(&mut self, can_commit: bool, f: F) -> T
	where
		F: FnOnce(&mut Self) -> T,
	{
		let mut clone;

		let reader = if can_commit {
			self
		} else {
			clone = self.clone();
			&mut clone
		};

		f(reader)
	}

	/// A lookahead which only commits its result if `can_commit` is true.
	fn lookahead_commit_if<T, F>(&mut self, can_commit: bool, f: F) -> T
	where
		F: FnOnce(&mut Self) -> T,
		T: LookaheadResult,
	{
		if can_commit {
			self.lookahead(f)
		} else {
			f(&mut self.clone())
		}
	}

	/// Returns an iterator which drains the remaining atoms from the cursor.
	fn drain(&mut self) -> CursorDrain<&'_ mut Self> {
		CursorDrain::new(self)
	}

	/// Consume `n` atoms and ignore them. Useful for recovery.
	fn skip(&mut self, n: usize) {
		for _ in self.drain().take(n) {}
	}

	/// Returns an iterator which yields the remaining atoms from the cursor without modifying the
	/// original cursor's state.
	fn peek_remaining(&self) -> CursorDrain<Self> {
		CursorDrain::new(self.clone())
	}
}

pub trait Atom: Clone {
	/// Returns true if the given atom is a stream delimiter (i.e. a start of file or end of file
	/// atom).
	fn is_stream_delimiter(&self) -> bool;
}

pub trait LookaheadResult {
	fn should_commit(&self) -> bool;
}

impl LookaheadResult for bool {
	fn should_commit(&self) -> bool {
		*self
	}
}

impl<T> LookaheadResult for Option<T> {
	fn should_commit(&self) -> bool {
		self.is_some()
	}
}

impl<T, E> LookaheadResult for Result<T, E> {
	fn should_commit(&self) -> bool {
		self.is_ok()
	}
}

pub struct BranchMatcher<'a, C: ForkableCursor, R> {
	target: &'a mut C,
	matched_cursor: Option<C>,
	result: R,
	barrier_has_val: bool,
}

impl<'a, C: ForkableCursor, R> BranchMatcher<'a, C, R> {
	pub fn case_proc<F, R2>(&mut self, f: F)
	where
		F: FnOnce(&mut C) -> R2,
		R2: LookaheadResult,
		R2: Into<R>,
	{
		let mut fork = self.target.clone();
		let result = f(&mut fork);

		if result.should_commit() {
			// Checks to ensure that we don't have an ambiguous fork
			assert!(!self.barrier_has_val, "ambiguous fork");
			self.barrier_has_val = true;

			// Result committing
			if self.matched_cursor.is_none() {
				self.matched_cursor = Some(fork);
				self.result = result.into();
			}
		}
	}

	pub fn barrier_proc(&mut self) {
		self.barrier_has_val = false;
	}

	pub fn case<F, R2>(mut self, f: F) -> Self
	where
		F: FnOnce(&mut C) -> R2,
		R2: LookaheadResult,
		R2: Into<R>,
	{
		self.case_proc(f);
		self
	}

	pub fn barrier(mut self) -> Self {
		self.barrier_proc();
		self
	}

	pub fn finish(self) -> R {
		if let Some(fork) = self.matched_cursor {
			*self.target = fork;
		}

		self.result
	}
}

#[derive(Debug, Clone)]
pub struct CursorDrain<C> {
	pub cursor: C,
	returned_eof: bool,
}

impl<C> CursorDrain<C> {
	pub fn new(cursor: C) -> Self {
		Self {
			cursor,
			returned_eof: false,
		}
	}
}

impl<C: Cursor> Iterator for CursorDrain<C> {
	type Item = (C::Loc, C::Atom);

	fn next(&mut self) -> Option<Self::Item> {
		if self.returned_eof {
			return None;
		}

		let (loc, atom) = self.cursor.consume();
		if atom.is_stream_delimiter() {
			self.returned_eof = true;
		}

		Some((loc, atom))
	}
}

// === PResult === //

pub type PResult<T> = Result<T, ParseError>;

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to match grammar")]
pub struct ParseError;

#[sealed]
pub trait ParsingErrorExt: OptionLike {
	fn or_recoverable<C, M>(
		self,
		recovery: &CursorRecovery<C, M>,
	) -> Result<Self::Value, CursorRecovery<C, M>>
	where
		C: ForkableCursor,
		M: Clone,
	{
		self.raw_option().ok_or_else(|| recovery.clone())
	}
}

#[sealed]
impl<T, C, M> ParsingErrorExt for Result<T, CursorRecovery<C, M>> {}

#[sealed]
impl<T> ParsingErrorExt for PResult<T> {}

#[sealed]
impl<T> ParsingErrorExt for Option<T> {}

// === CursorRecovery === //

#[derive(Debug, Clone)]
pub struct CursorRecovery<C, M> {
	pub furthest_cursor: C,
	pub meta: M,
}

impl<C: ForkableCursor, M> CursorRecovery<C, M> {
	pub fn new(cursor: &C, meta: M) -> Self {
		Self {
			furthest_cursor: cursor.clone(),
			meta,
		}
	}

	pub fn new_one_token_after(cursor: &C, meta: M) -> Self {
		let mut cursor = cursor.clone();
		let _ = cursor.consume();

		Self {
			furthest_cursor: cursor,
			meta,
		}
	}

	pub fn propose(&mut self, cursor: &C, meta: M) {
		if cursor.latest_loc() > self.furthest_cursor.latest_loc() {
			self.furthest_cursor = cursor.clone();
			self.meta = meta;
		}
	}

	pub fn recover(&self, cursor: &mut C) {
		debug_assert!(cursor.latest_loc() <= self.furthest_cursor.latest_loc());
		*cursor = self.furthest_cursor.clone();
	}
}

pub trait CursorRecoveryExt: ResultLike<Error = CursorRecovery<Self::Cursor, Self::Meta>> {
	type Cursor: ForkableCursor;
	type Meta;

	fn push_recovery<M, F>(
		self,
		recovery: &mut CursorRecovery<Self::Cursor, M>,
		xform_meta: F,
	) -> Result<Self::Success, ParseError>
	where
		F: FnOnce(Self::Meta) -> M,
	{
		self.raw_result().map_err(|err| {
			let meta = xform_meta(err.meta);
			recovery.propose(&err.furthest_cursor, meta);
			ParseError
		})
	}
}

impl<T, C: ForkableCursor, M> CursorRecoveryExt for Result<T, CursorRecovery<C, M>> {
	type Cursor = C;
	type Meta = M;
}

// === CursorUnstuck === //

/// A context object that provides [ForkableCursor]s with a way to display "unstuck" diagnostics of
/// the following form to the user:
///
/// ```text
/// do_something_with_float(0xFFe3);
///                             ^^
/// Unexpected floating-point exponent or start of identifier.
///   hint: hexadecimal numeric literals cannot specify floating-point
///         exponents
///   hint: identifiers cannot appear directly after a numeric literal
///
///   Expected a hexadecimal digit (0-9, A-F, or a separating underscore), a punctuation mark,
///            a whitespace, or a group delimiter (")").
/// ```
///
/// Unstuck diagnostics are generated by keeping track of the furthest (non-temporary) location of
/// the parsing cursor and pushing information about its location. There are three types of
/// information we can provide about a given location:
///
/// - Potential name*s* for the logical item appearing in front of the cursor (e.g. in the sample
///   diagnostic message, these would be "floating-point exponent" and "start of identifier")
/// - Hints explaining to the user why the parser may not have accepted their syntax (e.g. in the
///   sample diagnostic message, these would be "hexadecimal numeric literals cannot specify
///   floating-point exponents" and "identifiers cannot appear directly after a numeric literal")
/// - Things the user could type to continue the parsing process. (e.g. in the sample diagnostic
///   message, these would be "a hexadecimal digit (0-9, A-F, or a separating underscore)", "a
///   punctuation mark", "a whitespace", or "a group delimiter")
///
/// Because unstuck diagnostics work by specifying how to get a parser to continue parsing, grammars
/// which reject input based off the *presence* rather than the *lack* of a given sequence may not
/// work correctly. Consider, for example, an integer literal parser which rejects the integer
/// literal if a floating-point exponent is specified for a hexadecimal number:
///
/// ```rust
/// fn match_num_lit<'f>(
///     unstuck: CursorUnstuck<FileReader<'f>>,
///     reader: &mut FileReader<'f>,
/// ) -> PResult<TokenNumLit> {
///     reader.lookahead(|reader| {
///         // Match prefix
///         // (bumps unstuck cursor by expecting the `0b`, `0o`, and `0x` prefixes)
///         let prefix = match_num_prefix(unstuck, reader);  // Returns an `Option<NumLitPrefix>`.
///
///         // Match main digits
///         // (bumps unstuck cursor by expecting a digit)
///         let digits = match_digits_for_prefix(unstuck, reader, prefix);  // Returns a `String`.
///
///         if digits.is_empty() {
///             if prefix.is_some() {
///                 unstuck.hint("numeric literals with an explicit exponent must have at least one digit");
///             }
///             return Err(ParseError);
///         }
///
///         // Match floating-point exponent
///         let exp = {
///             let unstuck_builder = unstuck.bump();
///
///             if prefix.is_none() {
///                 // Only expect an exponent if we have no prefix.
///                 unstuck_builder.expect("e");
///             }
///
///             if match_char(reader, 'e').is_ok() {
///                 if let Some(prefix) = prefix {
///                     // Reject numbers with floating-point exponents in incorrect contexts.
///                     unstuck_builder.hint("cannot specify a floating-point exponent for a {prefix:?} prefixed number.");
///                     return Err(ParseError);
///                 }
///
///                 // Otherwise, match all the digits of the exponent
///                 Some(match_digits_for_prefix(unstuck, reader, None))
///             } else {
///                 None
///             }
///         };
///     })
/// }
/// ```
///
/// If the main tokenizer routine is implemented by `lookahead_fork`'ing over the various tokenized
/// elements and emitting an unstuck diagnostic if none of them match, the following input will
/// produce incomplete diagnostics:
///
/// ```text
/// do_something_with_float(0xFFe3);
///                             ^^
/// Unexpected.
///   hint: cannot specify a floating-point exponent for a hexadecimal prefixed number.
///
///   Expected a hexadecimal digit (0-9, A-F, or a separating underscore).
/// ```
///
/// The diagnostic fails to specify that we can actually get ourselves unstuck by specifying another
/// token. This happens because the tokenizer never actually tries to tokenize the token after the
/// invalid number, meaning that the tokenizer unstuck proposals are never pushed to the expectation
/// list.
///
/// Fundamentally, this happens because we break the assumption that parsing will only fail as the
/// result of all branches failing to recognize the input. When we reject an item based off the
/// *presence* rather than the *lack* of a syntactic element, we're prematurely preventing all other
/// branches from giving a shot at parsing.
///
/// If we stopped rejecting numbers with floating-point exponents in incorrect contexts—instead
/// treating them as if they lacked an exponent prefix entirely—diagnostics would function
/// properly:
///
/// ```no_run
/// // ...
///
/// // Match floating-point exponent
/// let exp = {
///     let unstuck_builder = unstuck.bump();
///
///     if prefix.is_none() {
///         // Only expect an exponent if we have no prefix.
///         unstuck_builder.expect("e");
///     }
///
///     reader.commit_if(prefix.is_none(), |reader| {
///         // Match exponent character
///         if match_char(reader, 'e').is_err() {
///             return None;
///         }
///
///         // Warn people about specifying exponents in improper contexts.
///         if let Some(prefix) = prefix {
///             unstuck_builder.hint("cannot specify a floating-point exponent for a {prefix:?} prefixed number");
///             return None;
///         }
///
///         // If this is a valid context, match all the digits of the exponent.
///         Some(match_digits_for_prefix(unstuck, reader, None))
///     })
/// };
///
/// // ...
/// ```
///
/// Running this updated routine on the same input, we get:
///
/// ```text
/// do_something_with_float(0xFFe3);
///                             ^^
///
/// Unexpected.
///   hint: cannot specify a floating-point exponent for a hexadecimal prefixed number
///   hint: identifiers cannot appear directly after a numeric literal
///
///   Expected a hexadecimal digit (0-9, A-F, or a separating underscore), a punctuation mark,
///            a whitespace, or a group delimiter (")").
/// ```
///
/// Of course, this relies on the main tokenizer rejecting identifiers that occur immediately after
/// a numeric literal.
#[derive(Debug)]
pub struct CursorUnstuck<C> {
	report: Option<UnstuckReport<C>>,
	locked: bool,
}

impl<C> Default for CursorUnstuck<C> {
	fn default() -> Self {
		Self {
			report: None,
			locked: false,
		}
	}
}

impl<C: ForkableCursor> CursorUnstuck<C> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn null() -> Self {
		Self {
			report: None,
			locked: true,
		}
	}

	pub fn bump(&mut self, cursor: &C) -> OptionalUnstuckReport<C> {
		if self.locked {
			return OptionalUnstuckReport { inner: None };
		}

		enum AltResult {
			CmpLess,
			CmpGreater,
		}

		let res = try_transform_or_err(&mut self.report, |report| {
			match report.as_mut().map(|report| {
				let cmp = report
					.furthest_cursor
					.latest_loc()
					.cmp(&cursor.latest_loc());

				(report, cmp)
			}) {
				Some((_, Ordering::Less)) | None => Err(AltResult::CmpLess),
				Some((report, Ordering::Equal)) => Ok(report),
				Some((_, Ordering::Greater)) => Err(AltResult::CmpGreater),
			}
		});

		match res {
			Ok(report) => OptionalUnstuckReport {
				inner: Some(report),
			},
			Err((slot, AltResult::CmpLess)) => {
				let report = slot.insert(UnstuckReport::new(cursor.clone()));

				OptionalUnstuckReport {
					inner: Some(report),
				}
			}
			Err((_, AltResult::CmpGreater)) => OptionalUnstuckReport { inner: None },
		}
	}

	pub fn encountered<D>(&mut self, cursor: &C, what: D)
	where
		D: ToString,
	{
		self.bump(cursor).encountered(what);
	}

	pub fn rejection_hint<D>(&mut self, cursor: &C, what: D)
	where
		D: ToString,
	{
		self.bump(cursor).rejection_hint(what);
	}

	pub fn expect<D: ToString>(&mut self, cursor: &C, what: D) {
		self.bump(cursor).expect(what);
	}

	pub fn expect_many<D: ToString, I: IntoIterator<Item = D>>(&mut self, cursor: &C, what: I) {
		let mut bumped = self.bump(cursor);

		for what in what {
			bumped.expect(what);
		}
	}
}

impl<C> CursorUnstuck<C> {
	pub fn unstuck_hint(&self) -> Option<&UnstuckReport<C>> {
		self.report.as_ref()
	}
}

pub struct OptionalUnstuckReport<'a, C> {
	pub inner: Option<&'a mut UnstuckReport<C>>,
}

impl<'a, C> OptionalUnstuckReport<'a, C> {
	pub fn is_some(&self) -> bool {
		self.inner.is_some()
	}

	pub fn encountered<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = &mut self.inner {
			inner.encountered(what);
		}
		self
	}

	pub fn rejection_hint<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = &mut self.inner {
			inner.rejection_hint(what);
		}
		self
	}

	pub fn expect<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = &mut self.inner {
			inner.expect(what);
		}
		self
	}
}

#[derive(Debug, Clone)]
pub struct UnstuckReport<C> {
	pub furthest_cursor: C,
	pub encountered: Vec<String>,
	pub rejection_hints: Vec<String>,
	pub expected: Vec<String>,
}

impl<C> UnstuckReport<C> {
	pub fn new(cursor: C) -> Self {
		Self {
			furthest_cursor: cursor,
			expected: Vec::new(),
			rejection_hints: Vec::new(),
			encountered: Vec::new(),
		}
	}

	pub fn encountered<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		self.encountered.push(what.to_string());
		self
	}

	pub fn rejection_hint<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		self.rejection_hints.push(what.to_string());
		self
	}

	pub fn expect<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		self.expected.push(what.to_string());
		self
	}
}
