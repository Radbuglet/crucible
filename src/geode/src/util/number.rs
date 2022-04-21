use derive_where::derive_where;
use hibitset::{BitSet, BitSetLike};
use std::error::Error;
use std::fmt::Display;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::replace;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

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

	pub fn as_option(self) -> Option<usize> {
		match self {
			OptionalUsize { raw: usize::MAX } => None,
			OptionalUsize { raw: value } => Some(value),
		}
	}
}

// === Number Generation === //

// Traits
pub trait NumberGenBase: Sized {
	type Value: Sized + Debug;

	fn generator_limit() -> Self::Value;
}

pub trait NumberGenRef: NumberGenBase {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>>;
}

pub trait NumberGenMut: NumberGenBase {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>>;
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct GenOverflowError<D> {
	_ty: PhantomData<D>,
}

impl<D> GenOverflowError<D> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<D: NumberGenBase> Error for GenOverflowError<D> {}

impl<D: NumberGenBase> Display for GenOverflowError<D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		writeln!(
			f,
			"generator overflowed (more than {:?} identifiers generated)",
			D::generator_limit(),
		)
	}
}

// Primitive generators
impl NumberGenBase for u64 {
	type Value = u64;

	fn generator_limit() -> Self::Value {
		u64::MAX
	}
}

impl NumberGenMut for u64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(replace(
			self,
			self.checked_add(1).ok_or(GenOverflowError::new())?,
		))
	}
}

impl NumberGenBase for NonZeroU64 {
	type Value = NonZeroU64;

	fn generator_limit() -> Self::Value {
		NonZeroU64::new(u64::MAX).unwrap()
	}
}

impl NumberGenMut for NonZeroU64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(replace(
			self,
			NonZeroU64::new(self.get().checked_add(1).ok_or(GenOverflowError::new())?).unwrap(),
		))
	}
}

impl NumberGenBase for AtomicU64 {
	type Value = u64;

	fn generator_limit() -> Self::Value {
		u64::MAX - 1000
	}
}

impl NumberGenRef for AtomicU64 {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self.fetch_add(1, Ordering::Relaxed);

		// Look, unless we manage to allocate more than `1000` IDs before this check runs, this check
		// is *perfectly fine*.
		if id > Self::generator_limit() {
			self.store(Self::generator_limit(), Ordering::Relaxed);
			return Err(GenOverflowError::new());
		}

		Ok(id)
	}
}

impl NumberGenMut for AtomicU64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		if *self.get_mut() >= Self::generator_limit() {
			return Err(GenOverflowError::new());
		} else {
			let next = *self.get_mut() + 1;
			Ok(replace(self.get_mut(), next))
		}
	}
}

#[derive(Debug)]
pub struct NonZeroU64Generator {
	pub counter: AtomicU64,
}

impl Default for NonZeroU64Generator {
	fn default() -> Self {
		Self {
			counter: AtomicU64::new(1),
		}
	}
}

impl NonZeroU64Generator {
	pub fn next_value(&mut self) -> NonZeroU64 {
		NonZeroU64::new(*self.counter.get_mut()).unwrap()
	}
}

impl NumberGenBase for NonZeroU64Generator {
	type Value = NonZeroU64;

	fn generator_limit() -> Self::Value {
		NonZeroU64::new(AtomicU64::generator_limit()).unwrap()
	}
}

impl NumberGenRef for NonZeroU64Generator {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self
			.counter
			.try_generate_ref()
			.ok()
			.ok_or(GenOverflowError::new())?;

		Ok(NonZeroU64::new(id).unwrap())
	}
}

impl NumberGenMut for NonZeroU64Generator {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self
			.counter
			.try_generate_mut()
			.ok()
			.ok_or(GenOverflowError::new())?;

		Ok(NonZeroU64::new(id).unwrap())
	}
}

// === Bit-level fun === //

pub const MAX_HIBITSET_INDEX: u32 = {
	let size = // The number of indices per layer2 bit
	BitSet::LAYER2_GRANULARITY *
		// The number of indices per layer2 word
		BitSet::BITS_PER_USIZE *
		// The number of indices that can be stored in the hibitset, given restrictions on layer3
		// size.
		BitSet::BITS_PER_USIZE;

	// TODO: What if usize is u16?! We really have to be careful with these casts!
	assert!(size < u32::MAX as usize);

	size as u32
};

pub fn is_valid_hibitset_index(index: usize) -> bool {
	let value = match u32::try_from(index) {
		Ok(value) => value,
		Err(_) => return false,
	};

	value <= MAX_HIBITSET_INDEX
}

pub const fn u64_msb_mask(offset: u32) -> u64 {
	debug_assert!(offset < 64);
	1u64.rotate_right(offset + 1)
}

pub const fn u64_has_mask(value: u64, mask: u64) -> bool {
	value | mask == value
}

pub fn hibitset_max_set_bit(set: &impl BitSetLike) -> Option<u32> {
	pub fn max_word_bit(word: usize) -> usize {
		// Words are read in the reverse direction
		// (leading) 0000 ... 0000 (trailing)
		//                       ^ index `0` according to hibitset
		//           ^ index BITS_PER_USIZE according to hibitset
		//
		// - If we have 0 leading zeros, the highest bit (`BitSet::BITS_PER_USIZE - 1`) must have
		//   been set.
		// - If we have `BitSet::BITS_PER_USIZE - 1` leading zeros, bit `0` must have been set.
		// - If we have `BitSet::BITS_PER_USIZE` leading zeros, the word is zero and should not be
		//   considered.

		debug_assert_ne!(word, 0);
		BitSet::BITS_PER_USIZE - 1 - word.leading_zeros() as usize
	}

	// Unfortunately, downstream crates can't rely on `BitSets` to smartly constrict to a minimum size
	// when bits at the end of the storage have been removed.

	if set.is_empty() {
		return None;
	}

	// Compute the index of the last l2 word with a set bit.
	// Each bit of l3 corresponds to a word of l2.
	let l2_max_word_index = max_word_bit(set.layer3());

	// Compute the index of the highest set bit in l2.
	// Each bit of l2 corresponds to a word of l1.
	let l1_max_word_index =
		// Index of the first bit in the word.
		BitSet::BITS_PER_USIZE * l2_max_word_index +
			// Index of the highest bit in that word.
			max_word_bit(set.layer2(l2_max_word_index));

	// Repeat the process for l1 to l0
	let l0_max_word_index =
		BitSet::BITS_PER_USIZE * l1_max_word_index + max_word_bit(set.layer1(l1_max_word_index));

	// Compute the highest index in layer0
	let l0_max_bit_index =
		BitSet::BITS_PER_USIZE * l0_max_word_index + max_word_bit(set.layer0(l0_max_word_index));

	// BitSet guarantees that its indices will never be greater than u32::MAX.
	Some(l0_max_bit_index as u32)
}

pub fn hibitset_length(set: &impl BitSetLike) -> u32 {
	hibitset_max_set_bit(set).map_or(0, |max_index|
		// This cannot overflow because u32::MAX is never a valid index in the bitset.
		max_index + 1)
}

#[cfg(test)]
mod tests {
	use super::*;
	use fastrand::usize;

	#[test]
	fn test_max_bitset() {
		let mut mirror = Vec::new();
		let mut set = BitSet::new();

		fastrand::seed(1000);

		for step in 1..=10_000 {
			print!("Step {step}: ");

			loop {
				match fastrand::bool() {
					true => {
						let value = fastrand::u32(0..MAX_HIBITSET_INDEX);
						if set.add(value) {
							continue;
						}

						println!("adding {value} to the set.");

						let insert_at = mirror.binary_search(&value).unwrap_err();
						mirror.insert(insert_at, value);
						break;
					}
					false => {
						if mirror.is_empty() {
							continue;
						}

						let value = mirror.remove(usize(0..mirror.len()));
						set.remove(value);

						println!("removing {value} from the set.");
						break;
					}
				}
			}

			let set_max = hibitset_max_set_bit(&set);
			let mirror_max = mirror.last().copied();

			assert_eq!(
				set_max, mirror_max,
				"set_max {set_max:?} != mirror_max {mirror_max:?}"
			);
		}
	}
}
