//! A module that does everything in its power to avoid forking hibitset.

use hibitset::{BitIter, BitSet, BitSetLike};

pub const LAYER2_WORD_COUNT: usize = BitSet::BITS_PER_USIZE;
pub const LAYER1_WORD_COUNT: usize = LAYER2_WORD_COUNT * BitSet::BITS_PER_USIZE;
pub const LAYER0_WORD_COUNT: usize = LAYER1_WORD_COUNT * BitSet::BITS_PER_USIZE;

pub const MAX_HIBITSET_INDEX_EXCLUSIVE: u32 = {
	let size = // The number of indices per layer2 bit
		BitSet::LAYER2_GRANULARITY *
			// The number of indices per layer2 word
			BitSet::BITS_PER_USIZE *
			// The number of indices that can be stored in the hibitset, given restrictions on layer3
			// size.
			BitSet::BITS_PER_USIZE;

	assert!(size < u32::MAX as usize);

	size as u32
};

pub fn is_valid_hibitset_index(index: usize) -> bool {
	let value = match u32::try_from(index) {
		Ok(value) => value,
		Err(_) => return false,
	};

	value <= MAX_HIBITSET_INDEX_EXCLUSIVE
}

pub fn hibitset_min_set_bit(set: &impl BitSetLike) -> Option<u32> {
	pub fn min_word_bit(word: usize) -> usize {
		// Words are read in the reverse direction:
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
		word.trailing_zeros() as usize
	}

	// Unfortunately, downstream crates can't rely on `BitSets` to smartly constrict to a minimum size
	// when bits at the end of the storage have been removed.

	if set.is_empty() {
		return None;
	}

	// Compute the index of the last l2 word with a set bit.
	// Each bit of l3 corresponds to a word of l2.
	let l2_min_word_index = min_word_bit(set.layer3());

	// Compute the index of the highest set bit in l2.
	// Each bit of l2 corresponds to a word of l1.
	let l1_min_word_index =
		// Index of the first bit in the word.
		BitSet::BITS_PER_USIZE * l2_min_word_index +
			// Index of the highest bit in that word.
			min_word_bit(set.layer2(l2_min_word_index));

	// Repeat the process for l1 to l0
	let l0_min_word_index =
		BitSet::BITS_PER_USIZE * l1_min_word_index + min_word_bit(set.layer1(l1_min_word_index));

	// Compute the highest index in layer0
	let l0_min_bit_index =
		BitSet::BITS_PER_USIZE * l0_min_word_index + min_word_bit(set.layer0(l0_min_word_index));

	// BitSet guarantees that its indices will never be greater than u32::MAX.
	Some(l0_min_bit_index as u32)
}

#[derive(Debug, Clone)]
pub struct BitSetRev<A: BitSetLike>(pub A);

impl<A: BitSetLike> BitSetLike for BitSetRev<A> {
	fn layer3(&self) -> usize {
		self.0.layer3().reverse_bits()
	}

	fn layer2(&self, i: usize) -> usize {
		self.0.layer2(LAYER2_WORD_COUNT - 1 - i).reverse_bits()
	}

	fn layer1(&self, i: usize) -> usize {
		self.0.layer1(LAYER1_WORD_COUNT - 1 - i).reverse_bits()
	}

	fn layer0(&self, i: usize) -> usize {
		self.0.layer0(LAYER0_WORD_COUNT - 1 - i).reverse_bits()
	}

	fn contains(&self, i: u32) -> bool {
		self.0.contains(reverse_hibitset_index(i))
	}
}

pub fn reverse_hibitset_index(index: u32) -> u32 {
	MAX_HIBITSET_INDEX_EXCLUSIVE - 1 - index
}

pub fn hibitset_iter_rev(set: &impl BitSetLike) -> impl Iterator<Item = u32> + '_ {
	let rev = BitSetRev(set);
	rev.iter().map(reverse_hibitset_index)
}

pub fn hibitset_max_set_bit(set: &impl BitSetLike) -> Option<u32> {
	let index = hibitset_min_set_bit(&BitSetRev(set)).map(reverse_hibitset_index);

	#[cfg(debug_assertions)]
	{
		let mut iter = hibitset_iter_rev(set);
		let official_index = iter.next();
		debug_assert_eq!(index, official_index);
	}
	index
}

pub fn hibitset_length(set: &impl BitSetLike) -> u32 {
	hibitset_max_set_bit(set).map_or(0, |max_index|
		// This cannot overflow because u32::MAX is never a valid index in the bitset.
		max_index + 1)
}

pub fn hibitset_iter_from<A: BitSetLike>(set: A, start_inclusive: u32) -> BitIter<A> {
	// To understand `masks` and `prefix`, one must understand hibitset's iteration algorithm:
	//
	// `masks` and `prefix` are ordered as l0, l1, l2, l3, although `prefix` omits l3.
	//
	// `masks` represent the current remaining bitmasks over which we're iterating. Every iteration
	// takes a bit from the lowest `mask`. If the mask is empty, it moves up the hierarchy looking
	// for a non-empty bit indicating the next word from which the lower layer should grab data.
	//
	// `prefix` contains a number per layer (excluding l3) which, when bitwise OR'd with the index
	// of the free bit of the current layer's mask, yields the index of the corresponding word in the
	// layer down below. In other words, each `prefix` is the bit index of the left-most bit of that
	// given word.

	fn mask_bits_geq(index: usize) -> usize {
		// Words are read in the reverse direction:
		// (leading) 0000 ... 0000 (trailing)
		//                       ^ index `0` according to hibitset
		//           ^ index BITS_PER_USIZE according to hibitset
		//
		// Thus, to mask out bit 0, we take 1 << index:
		//
		// (leading) 0000 ... 10 ... 0000 (trailing)
		//                    ^ index
		//
		// ...we subtract 1:
		//
		// (leading) 0000 ... 01 ... 1111 (trailing)
		//                    ^ index
		//
		// and then we invert the bits:
		//
		// (leading) 1111 ... 10 ... 0000 (trailing)
		//                    ^ index
		//
		// We have two (literal) potentially problematic cases: `index == 0` and `index == BITS_PER_USIZE - 1`:
		//
		// `index == 0` works properly because `!((1 << 0) - 1))` == `!(1-1)` == `0b1111...1111` and
		// `index == BITS_PER_USIZE - 1` because `(1 << (BITS_PER_USIZE - 1))` does not overflow,
		// with the same logic as above applying with no special casing.

		!((1 << index) - 1)
	}

	fn mask_bits_ge(index: usize) -> usize {
		mask_bits_geq(index) << 1
	}

	let l0_word_idx = start_inclusive as usize / BitSet::BITS_PER_USIZE;
	let l0_bit_idx = start_inclusive as usize % BitSet::BITS_PER_USIZE;

	let l1_word_idx = l0_word_idx / BitSet::BITS_PER_USIZE;
	let l1_bit_idx = l0_word_idx % BitSet::BITS_PER_USIZE;

	let l2_word_idx = l1_word_idx / BitSet::BITS_PER_USIZE;
	let l2_bit_idx = l1_word_idx % BitSet::BITS_PER_USIZE;

	// Yes, this behavior is technically different from .iter()'s when iterating from `0`. However,
	// if we're going to put in all the extra effort of finding all this data, we might as well use
	// it.
	let masks = [
		set.layer0(l0_word_idx) & mask_bits_geq(l0_bit_idx),
		// Higher masks don't need to iterate through their current word again, hence `ge` rather than
		// `geq`.
		set.layer1(l1_word_idx) & mask_bits_ge(l1_bit_idx),
		set.layer2(l2_word_idx) & mask_bits_ge(l2_bit_idx),
		set.layer3() & mask_bits_ge(l2_word_idx),
	];

	let prefix = [
		(l0_word_idx * BitSet::BITS_PER_USIZE) as u32,
		(l1_word_idx * BitSet::BITS_PER_USIZE) as u32,
		(l2_word_idx * BitSet::BITS_PER_USIZE) as u32,
	];

	BitIter::new(set, masks, prefix)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn init_seed() {
		let seed = fastrand::u64(..);
		fastrand::seed(seed);
		println!("Set seed to {seed}.");
	}

	fn bitset_mirror_add(mirror: &mut Vec<u32>, value: u32) {
		let insert_at = mirror.binary_search(&value).unwrap_err();
		mirror.insert(insert_at, value);
	}

	fn bitset_random_op(mirror: &mut Vec<u32>, set: &mut BitSet) {
		loop {
			match fastrand::bool() {
				true => {
					let value = fastrand::u32(0..MAX_HIBITSET_INDEX_EXCLUSIVE);
					if set.add(value) {
						continue;
					}

					println!("adding {value} to the set.");

					bitset_mirror_add(mirror, value);
					break;
				}
				false => {
					if mirror.is_empty() {
						continue;
					}

					let value = mirror.remove(fastrand::usize(0..mirror.len()));
					set.remove(value);

					println!("removing {value} from the set.");
					break;
				}
			}
		}
	}

	#[test]
	fn test_bitset_max() {
		init_seed();

		let mut mirror = Vec::new();
		let mut set = BitSet::new();

		for step in 1..=10_000 {
			print!("Step {step}: ");

			bitset_random_op(&mut mirror, &mut set);

			let set_max = hibitset_max_set_bit(&set);
			let mirror_max = mirror.last().copied();

			assert_eq!(set_max, mirror_max);
		}
	}

	#[test]
	fn test_bitset_iter() {
		init_seed();

		let mut mirror = Vec::new();
		let mut set = BitSet::new();

		for step in 1..=10_000 {
			print!("Step {step}: ");

			bitset_random_op(&mut mirror, &mut set);

			if mirror.len() > 0 {
				let start_at_index = fastrand::usize(0..mirror.len());
				let start_at_val = mirror[start_at_index];

				let mirror_iter = (&mirror[start_at_index..]).iter().copied();
				let bitset_iter = hibitset_iter_from(&set, start_at_val);

				assert!(
					mirror_iter.clone().eq(bitset_iter.clone()),
					"mirror iterator {:?} does not equal bitset iterator {:?}",
					mirror_iter.clone().collect::<Vec<_>>(),
					bitset_iter.clone().collect::<Vec<_>>(),
				);
			}
		}
	}
}
