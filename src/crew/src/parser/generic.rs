use sealed::sealed;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt::Display;
use std::ops::DerefMut;

/// A cursor is a view into a stream of elements (e.g. characters, tokens, etc) which we call *atoms*.
///
/// The cursor stream logically begins and ends with a delimiter. [Cursor::consume] shouldn't
/// emit this starting delimiter atom but [Cursor::latest] sure can if we haven't consumed anything
/// by the time it gets called. The ending delimiter, what we call an EOF, is said to be saturating
/// *i.e.* once an EOF is returned, the cursor should not return anything other than that EOF atom.
///
/// Smart mutable pointers to [Cursor] instances (*i.e.* objects that [DerefMut] to it) also
/// implement [Cursor].
pub trait Cursor: Sized {
	/// An atom yielded by the cursor. Delimiters (start and end of stream) are treated as atoms
	/// onto their own.
	type Atom: Atom;

	/// Returns the most recently returned location-atom pair. If nothing has been read yet, return
	/// the the left-hand delimiter atom, which should be the same atom kind as an EOF. If the EOF
	/// is consumed several times, calls to `latest` should return the EOF.
	fn latest(&self) -> Self::Atom;

	/// Consumes the next atom on the cursor head, replacing the state of [Cursor::latest] to
	/// reflect what it just returned (yes, this includes EOFs). Should not return the EOF representing
	/// the left delimiter of the stream. EOFs should be saturating *i.e.* after an EOF has been
	/// returned, the cursor should not return anything other than an EOF.
	fn consume(&mut self) -> Self::Atom;
}

impl<P: DerefMut<Target = T>, T: Cursor> Cursor for P {
	type Atom = T::Atom;

	fn latest(&self) -> Self::Atom {
		(&**self).latest()
	}

	fn consume(&mut self) -> Self::Atom {
		(&mut **self).consume()
	}
}

/// An extension trait applied to all [Cursor] instances that support [Clone] which adds utility
/// methods to consume the cursor contents with lookahead.
#[sealed]
pub trait ForkableCursor: Cursor + Clone {
	/// Return the next atom to be returned by the cursor without modifying the cursor state.
	fn peek(&self) -> Self::Atom {
		self.clone().consume()
	}

	/// Applies a match closure to a fork of the reader, only comitting the fork state when
	/// [LookaheadResult::should_commit] returns true.
	fn lookahead<F, R>(&mut self, mut f: F) -> R
	where
		F: FnMut(&mut Self) -> R,
		R: LookaheadResult,
	{
		let mut fork = self.clone();
		let res = f(&mut fork);
		if res.should_commit() {
			*self = fork;
		}
		res
	}

	/// Constructs a [BranchMatcher] builder object which allows the user to match against several
	/// grammars.
	fn branch<R>(&mut self) -> BranchMatcher<Self, R> {
		BranchMatcher {
			target: self,
			result: None,
			barrier_has_val: false,
		}
	}

	/// Returns an iterator which drains the remaining atoms from the cursor.
	fn drain_remaining<'a>(&'a mut self) -> CursorDrain<&'a mut Self> {
		CursorDrain(self)
	}

	/// Returns an iterator which yields the remaining atoms from the cursor without modifying the
	/// original cursor's state.
	fn peek_remaining(&self) -> CursorDrain<Self> {
		CursorDrain(self.clone())
	}
}

#[sealed]
impl<T: Cursor + Clone> ForkableCursor for T {}

pub trait Atom: Clone {
	/// Should return true if and only if the given atom is an ending delimiter.
	fn is_eof(&self) -> bool;
}

pub trait LookaheadResult {
	fn should_commit(&self) -> bool;
}

pub struct BranchMatcher<'a, C: ForkableCursor, R> {
	target: &'a mut C,
	result: Option<(C, R)>,
	barrier_has_val: bool,
}

impl<'a, C: ForkableCursor, R> BranchMatcher<'a, C, R> {
	pub fn case_proc<F, R2>(&mut self, mut f: F)
	where
		F: FnMut(&mut C) -> R2,
		R2: LookaheadResult,
		R2: Into<R>,
	{
		let mut fork = self.target.clone();
		let res = f(&mut fork);

		if res.should_commit() {
			// Debug checks to ensure that we don't have an ambiguous branch
			assert!(!self.barrier_has_val, "ambiguous branch");
			self.barrier_has_val = true;

			// Result committing
			if self.result.is_none() {
				self.result = Some((fork, res.into()));
			}
		}
	}

	pub fn barrier_proc(&mut self) {
		self.barrier_has_val = false;
	}

	pub fn case<F, R2>(mut self, f: F) -> Self
	where
		F: FnMut(&mut C) -> R2,
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

	pub fn finish(self) -> Option<R> {
		let (fork, res) = self.result?;
		*self.target = fork;
		Some(res)
	}
}

#[derive(Debug, Clone)]
pub struct CursorDrain<C>(pub C);

impl<C: Cursor> Iterator for CursorDrain<C> {
	type Item = C::Atom;

	fn next(&mut self) -> Option<Self::Item> {
		if self.0.latest().is_eof() {
			return None;
		}
		Some(self.0.consume())
	}
}

pub trait DiagnosticCursor: ForkableCursor
where
	Self::Atom: Eq + Ord,
{
	fn error_reporter(&self) -> &ErrorReporter<Self>;

	fn expect<D: Display>(&self, what: D) {
		let mut furthest = self.error_reporter().furthest.borrow_mut();

		match &mut *furthest {
			Some(report) => match report.furthest_cursor.latest().cmp(&self.latest()) {
				Ordering::Less => {
					*furthest = Some(ErrorReport {
						furthest_cursor: self.clone(),
						unstuck_options: vec![what.to_string()],
					});
				}
				Ordering::Equal => report.unstuck_options.push(what.to_string()),
				Ordering::Greater => { /* ignore report */ }
			},
			None => {
				*furthest = Some(ErrorReport {
					furthest_cursor: self.clone(),
					unstuck_options: vec![what.to_string()],
				});
			}
		}
	}
}

pub struct ErrorReporter<C: ForkableCursor> {
	furthest: RefCell<Option<ErrorReport<C>>>,
}

impl<C: ForkableCursor> Default for ErrorReporter<C> {
	fn default() -> Self {
		Self {
			furthest: Default::default(),
		}
	}
}

impl<C: ForkableCursor> ErrorReporter<C> {
	pub fn report(&mut self) -> Option<&ErrorReport<C>> {
		self.furthest.get_mut().as_ref()
	}
}

pub struct ErrorReport<C: ForkableCursor> {
	pub furthest_cursor: C,
	// TODO: We might want support for dynamic hints and nested error reports.
	pub unstuck_options: Vec<String>,
}
