use std::{cmp::Ordering, fmt, ops::DerefMut};

use crucible_core::std_traits::OptionLike;
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
/// implement [Cursor].
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

/// An extension trait applied to all [Cursor] instances that support [Clone] which adds utility
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
			#[cfg(debug_assertions)]
			barrier_has_val: false,
		}
	}

	/// Returns an iterator which drains the remaining atoms from the cursor.
	fn drain(&mut self) -> CursorDrain<&'_ mut Self> {
		CursorDrain(self)
	}

	/// Consume `n` atoms and ignore them. Useful for recovery.
	fn skip(&mut self, n: usize) {
		for _ in self.drain().take(n) {}
	}

	/// Returns an iterator which yields the remaining atoms from the cursor without modifying the
	/// original cursor's state.
	fn peek_remaining(&self) -> CursorDrain<Self> {
		CursorDrain(self.clone())
	}
}

impl<T: Cursor + Clone> ForkableCursor for T {}

pub trait Atom: Clone {
	/// Returns true if the given atom is an ending delimiter.
	fn is_eof(&self) -> bool;
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
	#[cfg(debug_assertions)]
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
			// Debug checks to ensure that we don't have an ambiguous branch
			#[cfg(debug_assertions)]
			{
				assert!(!self.barrier_has_val, "ambiguous branch");
				self.barrier_has_val = true;
			}

			// Result committing
			if self.matched_cursor.is_none() {
				self.matched_cursor = Some(fork);
				self.result = result.into();
			}
		}
	}

	pub fn barrier_proc(&mut self) {
		#[cfg(debug_assertions)]
		{
			self.barrier_has_val = false;
		}
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
pub struct CursorDrain<C>(pub C);

impl<C: Cursor> Iterator for CursorDrain<C> {
	type Item = (C::Loc, C::Atom);

	fn next(&mut self) -> Option<Self::Item> {
		if self.0.latest_atom().is_eof() {
			return None;
		}
		Some(self.0.consume())
	}
}

// === PResult === //

pub type PResult<T> = Result<T, ParseError>;

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to match grammar")]
pub struct ParseError;

impl<C, M> From<CursorRecovery<C, M>> for ParseError {
	fn from(_: CursorRecovery<C, M>) -> Self {
		Self
	}
}

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

// === CursorUnstuck === //

#[derive(Debug)]
pub struct CursorUnstuck<C> {
	furthest: Option<UnstuckReport<C>>,
}

#[derive(Debug)]
pub struct UnstuckReport<C> {
	pub furthest_cursor: C,
	pub unstuck_options: Vec<String>,
}

impl<C> Default for CursorUnstuck<C> {
	fn default() -> Self {
		Self { furthest: None }
	}
}

impl<C: ForkableCursor> CursorUnstuck<C> {
	pub fn expect_many<I, D>(&mut self, cursor: &C, what: I)
	where
		I: IntoIterator<Item = D>,
		D: fmt::Display,
	{
		for what in what {
			self.expect(cursor, what);
		}
	}

	pub fn expect<D: fmt::Display>(&mut self, cursor: &C, what: D) {
		match &mut self.furthest {
			Some(report) => match report
				.furthest_cursor
				.latest_loc()
				.cmp(&cursor.latest_loc())
			{
				Ordering::Less => {
					self.furthest = Some(UnstuckReport {
						furthest_cursor: cursor.clone(),
						unstuck_options: vec![what.to_string()],
					});
				}
				Ordering::Equal => report.unstuck_options.push(what.to_string()),
				Ordering::Greater => { /* ignore report */ }
			},
			None => {
				self.furthest = Some(UnstuckReport {
					furthest_cursor: cursor.clone(),
					unstuck_options: vec![what.to_string()],
				});
			}
		}
	}

	pub fn unstuck_hint(&self) -> Option<&UnstuckReport<C>> {
		self.furthest.as_ref()
	}
}
