use std::hash::Hash;
use std::ops::{BitAnd, BitAndAssign, Not};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Bitmask64(pub u64);

impl Bitmask64 {
	pub const EMPTY: Self = Self(u64::MIN);
	pub const FULL: Self = Self(u64::MAX);

	pub fn one_hot(bit: usize) -> Self {
		Bitmask64(1u64 << bit)
	}

	fn has_zero(&self) -> bool {
		*self != Self::FULL
	}

	fn has_one(&self) -> bool {
		*self != Self::EMPTY
	}

	pub fn add(&mut self, other: Self) {
		*self &= other;
	}

	pub fn remove(&mut self, other: Self) {
		*self &= !other;
	}

	pub fn reserve_flag(&mut self) -> Option<usize> {
		if self.has_one() {
			let index = self.0.leading_ones() as usize;
			self.add(Self::one_hot(index));
			Some(index)
		} else {
			None
		}
	}

	pub fn iter_zeros(&self) -> Bitmask64BitIter {
		Bitmask64BitIter::new(!*self)
	}

	pub fn iter_ones(&self) -> Bitmask64BitIter {
		Bitmask64BitIter::new(*self)
	}

	pub fn contains(&self, other: &Self) -> bool {
		self.0 & other.0 == other.0
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
			let next_one = self.curr.0.leading_zeros() as usize;
			self.curr.remove(Bitmask64::one_hot(next_one));
			Some(next_one)
		} else {
			None
		}
	}
}
