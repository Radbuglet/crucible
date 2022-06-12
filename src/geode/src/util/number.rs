use std::cmp::Ordering as CmpOrdering;
use std::error::Error;
use std::fmt::Display;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::mem::replace;
use std::num::NonZeroU64;
use std::ops::{Bound, RangeBounds};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::error::ResultExt;

// === OptionalUsize === //

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct OptionalUsize {
	pub raw: usize,
}

impl Debug for OptionalUsize {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self.as_option() {
			Some(value) => write!(f, "OptionalUsize::Some({value})"),
			None => write!(f, "OptionalUsize::None"),
		}
	}
}

impl Default for OptionalUsize {
	fn default() -> Self {
		Self::NONE
	}
}

impl OptionalUsize {
	pub const NONE: Self = Self { raw: usize::MAX };

	pub fn some(value: usize) -> Self {
		debug_assert!(value != usize::MAX);
		Self { raw: value }
	}

	pub fn as_option(self) -> Option<usize> {
		match self {
			OptionalUsize { raw: usize::MAX } => None,
			OptionalUsize { raw: value } => Some(value),
		}
	}
}

// === Number Generation === //

// Traits
pub trait NumberGenRef {
	type Value;
	type GenError: Error;

	fn try_generate_ref(&self) -> Result<Self::Value, Self::GenError>;

	fn generate_ref(&self) -> Self::Value {
		self.try_generate_ref().unwrap_pretty()
	}
}

pub trait NumberGenMut {
	type Value;
	type GenError: Error;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError>;

	fn generate_mut(&mut self) -> Self::Value {
		self.try_generate_mut().unwrap_pretty()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct GenOverflowError<D> {
	pub limit: D,
}

impl<D: Debug> Error for GenOverflowError<D> {}

impl<D: Debug> Display for GenOverflowError<D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		writeln!(
			f,
			"generator overflowed (more than {:?} identifiers generated)",
			self.limit,
		)
	}
}

// Primitive generators
#[derive(Debug, Clone, Default)]
pub struct U64Generator {
	pub next: u64,
}

impl NumberGenMut for U64Generator {
	type Value = u64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError> {
		let subsequent = self
			.next
			.checked_add(1)
			.ok_or(GenOverflowError { limit: u64::MAX })?;

		Ok(replace(&mut self.next, subsequent))
	}
}

#[derive(Debug, Clone)]
pub struct NonZeroU64Generator {
	pub next: NonZeroU64,
}

impl Default for NonZeroU64Generator {
	fn default() -> Self {
		Self {
			next: NonZeroU64::new(1).unwrap(),
		}
	}
}

impl NumberGenMut for NonZeroU64Generator {
	type Value = NonZeroU64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError> {
		let subsequent = NonZeroU64::new(
			self.next
				.get()
				.checked_add(1)
				.ok_or(GenOverflowError { limit: u64::MAX })?,
		)
		.unwrap();

		Ok(replace(&mut self.next, subsequent))
	}
}

#[derive(Debug, Default)]
pub struct AtomicU64Generator {
	pub next: AtomicU64,
}

const ATOMIC_U64_LIMIT: u64 = u64::MAX - 1000;

impl NumberGenRef for AtomicU64Generator {
	type Value = u64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_ref(&self) -> Result<Self::Value, Self::GenError> {
		let id = self.next.fetch_add(1, AtomicOrdering::Relaxed);

		// Look, unless we manage to allocate more than `1000` IDs before this check runs, this check
		// is *perfectly fine*.
		if id > ATOMIC_U64_LIMIT {
			self.next.store(ATOMIC_U64_LIMIT, AtomicOrdering::Relaxed);
			return Err(GenOverflowError {
				limit: ATOMIC_U64_LIMIT,
			});
		}

		Ok(id)
	}
}

impl NumberGenMut for AtomicU64Generator {
	type Value = u64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError> {
		if *self.next.get_mut() >= ATOMIC_U64_LIMIT {
			Err(GenOverflowError {
				limit: ATOMIC_U64_LIMIT,
			})
		} else {
			let next = *self.next.get_mut() + 1;
			Ok(replace(self.next.get_mut(), next))
		}
	}
}

#[derive(Debug)]
pub struct AtomicNZU64Generator {
	next: AtomicU64Generator,
}

impl Default for AtomicNZU64Generator {
	fn default() -> Self {
		Self {
			next: AtomicU64Generator {
				next: AtomicU64::new(1),
			},
		}
	}
}

impl AtomicNZU64Generator {
	pub fn next_value(&mut self) -> NonZeroU64 {
		NonZeroU64::new(*self.next.next.get_mut()).unwrap()
	}
}

impl NumberGenRef for AtomicNZU64Generator {
	type Value = NonZeroU64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_ref(&self) -> Result<Self::Value, Self::GenError> {
		self.next
			.try_generate_ref()
			.map(|id| NonZeroU64::new(id).unwrap())
	}
}

impl NumberGenMut for AtomicNZU64Generator {
	type Value = NonZeroU64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError> {
		self.next
			.try_generate_mut()
			.map(|id| NonZeroU64::new(id).unwrap())
	}
}

// === Range utilities === //

pub fn cmp_to_range<T: Ord, R: RangeBounds<T>>(value: &T, range: &R) -> CmpOrdering {
	let in_left = cmp_to_left_bound(range.start_bound(), value);
	let in_right = cmp_to_right_bound(value, range.end_bound());

	match (in_left, in_right) {
		(true, true) => CmpOrdering::Equal,
		(true, false) => CmpOrdering::Less,
		(false, true) => CmpOrdering::Greater,

		// Would require us to be both below a left bound, but somehow above a right bound,
		// implying an ill-formed range.
		(false, false) => unreachable!(),
	}
}

/// Roughly equivalent to:
///
/// ```ignore
/// (left..).contains(value)
/// ```
///
/// except that `left` can be an exclusive or unbounded bound.
fn cmp_to_left_bound<T: Ord>(left: Bound<&T>, value: &T) -> bool {
	match left {
		Bound::Included(left) => left.le(value),
		Bound::Excluded(left) => left.lt(value),
		Bound::Unbounded => true,
	}
}

/// Roughly equivalent to:
///
/// ```ignore
/// (..right).contains(value)
/// ```
///
/// except that `right` can be an inclusive or unbounded bound.
fn cmp_to_right_bound<T: Ord>(value: &T, right: Bound<&T>) -> bool {
	match right {
		Bound::Included(right) => value.le(right),
		Bound::Excluded(right) => value.lt(right),
		Bound::Unbounded => true,
	}
}
