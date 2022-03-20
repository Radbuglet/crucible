use derive_where::derive_where;
use std::error::Error;
use std::fmt::Display;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::sync::atomic::{AtomicU64, Ordering};

// === Bitmask64 === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct Bitmask64(pub u64);

impl Debug for Bitmask64 {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "Bitmask64({:#064b})", self.0)
	}
}

impl Bitmask64 {
	pub const EMPTY: Self = Self(u64::MIN);
	pub const FULL: Self = Self(u64::MAX);

	pub fn one_hot(bit: usize) -> Self {
		debug_assert!(bit < 64);
		Bitmask64(1u64 << bit)
	}

	pub fn is_empty(self) -> bool {
		self == Self::EMPTY
	}

	pub fn is_full(self) -> bool {
		self == Self::FULL
	}

	fn has_zero(self) -> bool {
		self != Self::FULL
	}

	fn has_one(self) -> bool {
		self != Self::EMPTY
	}

	pub fn is_set(self, index: usize) -> bool {
		(self & Self::one_hot(index)).has_one()
	}

	pub fn add(&mut self, other: Self) {
		*self |= other;
	}

	pub fn remove(&mut self, other: Self) {
		*self &= !other;
	}

	pub fn reserve_flag(&mut self) -> Option<usize> {
		if self.has_zero() {
			let index = self.0.trailing_ones() as usize;
			self.add(Self::one_hot(index));
			Some(index)
		} else {
			None
		}
	}

	pub fn contains(self, other: Self) -> bool {
		(self & other).has_one()
	}

	pub fn is_superset_of(self, other: Self) -> bool {
		self & other == other
	}

	pub fn iter_zeros(self) -> Bitmask64BitIter {
		Bitmask64BitIter::new(!self)
	}

	pub fn iter_ones(self) -> Bitmask64BitIter {
		Bitmask64BitIter::new(self)
	}
}

impl BitAnd for Bitmask64 {
	type Output = Self;

	fn bitand(self, rhs: Self) -> Self::Output {
		Self(self.0 & rhs.0)
	}
}

impl BitAndAssign for Bitmask64 {
	fn bitand_assign(&mut self, rhs: Self) {
		self.0 &= rhs.0;
	}
}

impl BitOr for Bitmask64 {
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0 | rhs.0)
	}
}

impl BitOrAssign for Bitmask64 {
	fn bitor_assign(&mut self, rhs: Self) {
		self.0 |= rhs.0;
	}
}

impl Not for Bitmask64 {
	type Output = Self;

	fn not(self) -> Self::Output {
		Self(!self.0)
	}
}

#[derive(Debug, Clone)]
pub struct Bitmask64BitIter {
	curr: Bitmask64,
}

impl Bitmask64BitIter {
	pub fn new(mask: Bitmask64) -> Self {
		Self { curr: mask }
	}
}

impl Iterator for Bitmask64BitIter {
	type Item = usize;

	fn next(&mut self) -> Option<Self::Item> {
		if self.curr.has_one() {
			let next_one = self.curr.0.trailing_zeros() as usize;
			self.curr.remove(Bitmask64::one_hot(next_one));
			Some(next_one)
		} else {
			None
		}
	}
}

// === OptionalUsize === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct OptionalUsize {
	pub raw: usize,
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

	pub fn wrap(value: Option<usize>) -> Self {
		match value {
			Some(value) => Self::some(value),
			None => Self::NONE,
		}
	}

	pub fn as_option(self) -> Option<usize> {
		match self {
			OptionalUsize { raw: usize::MAX } => None,
			OptionalUsize { raw: value } => Some(value),
		}
	}
}

// === Number Generation === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct GenOverflowError<D> {
	_ty: PhantomData<D>,
}

impl<D> GenOverflowError<D> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<D: NumberGenExt> Error for GenOverflowError<D> {}

impl<D: NumberGenExt> Display for GenOverflowError<D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		writeln!(
			f,
			"generator overflowed (more than {} identifiers generated)",
			D::limit(),
		)
	}
}

pub trait NumberGenExt: Sized {
	type Value: Sized + Display;

	fn limit() -> Self::Value;
	fn try_generate(self) -> Result<Self::Value, GenOverflowError<Self>>;
}

impl<'a> NumberGenExt for &'a mut u64 {
	type Value = u64;

	fn limit() -> Self::Value {
		u64::MAX
	}

	fn try_generate(self) -> Result<Self::Value, GenOverflowError<Self>> {
		self.checked_add(1).ok_or(GenOverflowError::new())
	}
}

impl<'a> NumberGenExt for &'a AtomicU64 {
	type Value = u64;

	fn limit() -> Self::Value {
		u64::MAX - 1000
	}

	fn try_generate(self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self.fetch_add(1, Ordering::Relaxed);

		// Look, unless we manage to allocate more than `1000` IDs before this check runs, this check
		// is *perfectly fine*.
		if id > Self::limit() {
			self.store(Self::limit(), Ordering::Relaxed);
			return Err(GenOverflowError::new());
		}

		Ok(id)
	}
}

// === usize bit-masking === //

pub const fn usize_msb_mask(offset: u32) -> usize {
	debug_assert!(offset < 64);
	1usize.rotate_right(offset + 1)
}

pub const fn usize_has_mask(value: usize, mask: usize) -> bool {
	value | mask == value
}
