use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::num::NonZeroU64;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct OptionalUsize(usize);

impl Default for OptionalUsize {
    fn default() -> Self {
        Self::NONE
    }
}

impl OptionalUsize {
    pub const NONE: Self = Self(usize::MAX);

    pub fn some(value: usize) -> Self {
        debug_assert!(value != usize::MAX);
        Self(value)
    }

    pub fn wrap(value: Option<usize>) -> Self {
        match value {
            Some(value) => Self::some(value),
            None => Self::NONE,
        }
    }

    pub fn unwrap(self) -> Option<usize> {
        match self {
            OptionalUsize(usize::MAX) => None,
            OptionalUsize(value) => Some(value),
        }
    }
}

pub trait NonZeroU64Ext {
    fn checked_add_assign(&mut self, other: u64) -> Option<NonZeroU64>;
}

impl NonZeroU64Ext for NonZeroU64 {
    fn checked_add_assign(&mut self, other: u64) -> Option<NonZeroU64> {
        *self = self.checked_add(other)?;
        Some(*self)
    }
}
