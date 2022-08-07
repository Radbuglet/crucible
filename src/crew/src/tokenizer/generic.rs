use std::{
	cmp::Ordering,
	ops::{Deref, DerefMut},
};

use bytemuck::TransparentWrapper;
use crucible_core::{
	lifetime::try_transform_or_err,
	std_traits::{OptionLike, ResultLike},
	wide_option::WideOption,
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

// === CursorAnnotator === //

#[derive(TransparentWrapper)]
#[repr(transparent)]
pub struct CursorAnnotatorOrDrain<C, A>(WideOption<CursorAnnotator<C, A>>);

#[derive(Debug, Clone, Default)]
pub struct CursorAnnotator<C, A> {
	pub cursor: C,
	pub annotation: A,
}

impl<C, A> CursorAnnotator<C, A>
where
	C: ForkableCursor,
{
	pub fn new_here(cursor: &C, annotation: A) -> Self {
		Self {
			cursor: cursor.clone(),
			annotation,
		}
	}

	pub fn new_here_default(cursor: &C) -> Self
	where
		A: Default,
	{
		Self {
			cursor: cursor.clone(),
			annotation: A::default(),
		}
	}

	pub fn new_one_after(cursor: &C, annotation: A) -> Self {
		let mut cursor = cursor.clone();
		let _ = cursor.consume();

		Self { cursor, annotation }
	}

	pub fn new_one_after_default(cursor: &C) -> Self
	where
		A: Default,
	{
		Self::new_one_after(cursor, A::default())
	}
}

impl<C, A> CursorAnnotator<C, A> {
	pub fn new_drain<'a>() -> &'a mut CursorAnnotatorOrDrain<C, A> {
		CursorAnnotatorOrDrain::wrap_mut(WideOption::none())
	}
}

impl<C, A> Deref for CursorAnnotator<C, A> {
	type Target = CursorAnnotatorOrDrain<C, A>;

	fn deref(&self) -> &Self::Target {
		CursorAnnotatorOrDrain::wrap_ref(WideOption::some(self))
	}
}

impl<C, A> DerefMut for CursorAnnotator<C, A> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		CursorAnnotatorOrDrain::wrap_mut(WideOption::some_mut(self))
	}
}

impl<C, A> CursorAnnotatorOrDrain<C, A> {
	pub fn is_real(&self) -> bool {
		self.0.is_some()
	}

	pub fn is_drain(&self) -> bool {
		self.0.is_none()
	}

	pub fn real(&self) -> Option<&CursorAnnotator<C, A>> {
		self.0.as_option()
	}

	pub fn real_mut(&mut self) -> Option<&mut CursorAnnotator<C, A>> {
		self.0.as_option_mut()
	}

	pub fn allow_bump_if(&mut self, cond: bool) -> &mut Self {
		if cond {
			self
		} else {
			CursorAnnotator::new_drain()
		}
	}
}

impl<C, A> CursorAnnotatorOrDrain<C, A>
where
	C: ForkableCursor,
{
	pub fn bump_with_fn<F>(&mut self, cursor: &C, factory: F) -> Option<&mut A>
	where
		F: FnOnce() -> A,
	{
		let me = match self.real_mut() {
			Some(me) => me,
			None => return None,
		};

		enum AltResult {
			CmpLess,
			CmpGreater,
		}

		let res = try_transform_or_err(&mut me.annotation, |annotation| {
			match me.cursor.latest_loc().cmp(&cursor.latest_loc()) {
				Ordering::Less => Err(AltResult::CmpLess),
				Ordering::Equal => Ok(annotation),
				Ordering::Greater => Err(AltResult::CmpGreater),
			}
		});

		match res {
			Ok(annotation) => Some(annotation),
			Err((slot, AltResult::CmpLess)) => {
				*slot = factory();
				Some(slot)
			}
			Err((_, AltResult::CmpGreater)) => None,
		}
	}

	pub fn bump_default(&mut self, cursor: &C) -> Option<&mut A>
	where
		A: Default,
	{
		self.bump_with_fn(cursor, A::default)
	}
}

// === CursorRecovery === //

pub type CursorRecovery<C, M> = CursorAnnotator<C, CursorRecoveryMeta<M>>;
pub type CursorRecoveryOrDrain<C, M> = CursorAnnotatorOrDrain<C, CursorRecoveryMeta<M>>;

#[derive(Debug, Clone, Default)]
pub struct CursorRecoveryMeta<M>(pub M);

impl<M> From<M> for CursorRecoveryMeta<M> {
	fn from(meta: M) -> Self {
		CursorRecoveryMeta(meta)
	}
}

pub trait CursorRecoveryExt {
	type Cursor;
	type Meta;

	fn propose(&mut self, cursor: &Self::Cursor, meta: Self::Meta);
	fn recover(&self, cursor: &mut Self::Cursor);
}

impl<C, M> CursorRecoveryExt for CursorRecovery<C, M>
where
	C: ForkableCursor,
{
	type Cursor = C;
	type Meta = M;

	fn propose(&mut self, cursor: &Self::Cursor, meta: Self::Meta) {
		self.bump_with_fn(cursor, || CursorRecoveryMeta(meta));
	}

	fn recover(&self, cursor: &mut Self::Cursor) {
		debug_assert!(cursor.latest_loc() <= self.cursor.latest_loc());
		*cursor = self.cursor.clone();
	}
}

pub trait CursorRecoveryResultExt:
	ResultLike<Error = CursorRecovery<Self::Cursor, Self::Meta>>
{
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
			let meta = xform_meta(err.annotation.0);
			recovery.propose(&err.cursor, meta);
			ParseError
		})
	}
}

impl<T, C: ForkableCursor, M> CursorRecoveryResultExt for Result<T, CursorRecovery<C, M>> {
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
pub type CursorUnstuck<C> = CursorAnnotator<C, UnstuckReport>;

pub type CursorUnstuckOrDrain<C> = CursorAnnotatorOrDrain<C, UnstuckReport>;

pub trait CursorUnstuckExt {
	type Cursor;

	fn encountered<D>(&mut self, cursor: &Self::Cursor, what: D)
	where
		D: ToString;

	fn rejection_hint<D>(&mut self, cursor: &Self::Cursor, what: D)
	where
		D: ToString;

	fn expects<D: ToString>(&mut self, cursor: &Self::Cursor, what: D);

	fn expects_many<D: ToString, I: IntoIterator<Item = D>>(
		&mut self,
		cursor: &Self::Cursor,
		what: I,
	);
}

impl<C: ForkableCursor> CursorUnstuckExt for CursorUnstuckOrDrain<C> {
	type Cursor = C;

	fn encountered<D>(&mut self, cursor: &C, what: D)
	where
		D: ToString,
	{
		self.bump_default(cursor).encountered(what);
	}

	fn rejection_hint<D>(&mut self, cursor: &C, what: D)
	where
		D: ToString,
	{
		self.bump_default(cursor).rejection_hint(what);
	}

	fn expects<D: ToString>(&mut self, cursor: &C, what: D) {
		self.bump_default(cursor).expects(what);
	}

	fn expects_many<D: ToString, I: IntoIterator<Item = D>>(&mut self, cursor: &C, what: I) {
		let mut bumped = self.bump_default(cursor);

		for what in what {
			bumped.expects(what);
		}
	}
}

pub trait CursorUnstuckReportExt {
	fn encountered<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString;

	fn rejection_hint<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString;

	fn expects<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString;
}

impl<'a> CursorUnstuckReportExt for Option<&'a mut UnstuckReport> {
	fn encountered<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = self {
			inner.encountered(what);
		}
		self
	}

	fn rejection_hint<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = self {
			inner.rejection_hint(what);
		}
		self
	}

	fn expects<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		if let Some(inner) = self {
			inner.expects(what);
		}
		self
	}
}

#[derive(Debug, Clone, Default)]
pub struct UnstuckReport {
	pub encountered: Vec<String>,
	pub rejection_hints: Vec<String>,
	pub expected: Vec<String>,
}

impl UnstuckReport {
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

	pub fn expects<D>(&mut self, what: D) -> &mut Self
	where
		D: ToString,
	{
		self.expected.push(what.to_string());
		self
	}
}
