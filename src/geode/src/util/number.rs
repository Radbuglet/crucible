use std::{
	num::NonZeroU64,
	sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

// === NonZeroU64 utilities === //

pub trait NonZeroNumExt {
	type Primitive;

	fn prim(self) -> Self::Primitive;
}

impl NonZeroNumExt for Option<NonZeroU64> {
	type Primitive = u64;

	fn prim(self) -> Self::Primitive {
		self.map_or(0, NonZeroU64::get)
	}
}

// === Bitset utilities === //

/// Reserves the least significant one bit from the `target`, marks it as a `0`, and returns its
/// index from the LSB.
///
/// Returns `64` if no bits could be allocated.
pub fn reserve_one_bit(target: &mut u64) -> u8 {
	let pos = target.trailing_zeros() as u8;
	unset_bit(target, pos);
	pos
}

/// Reserves the least significant zero bit from the `target`, marks it as a `1`, and returns its
/// index from the LSB.
///
/// Returns `64` if no bits could be allocated.
pub fn reserve_zero_bit(target: &mut u64) -> u8 {
	let pos = target.trailing_ones() as u8;
	set_bit(target, pos);
	pos
}

/// Sets the specified bit to `1`.
pub fn set_bit(target: &mut u64, pos: u8) {
	*target |= bit_mask(pos);
}

/// Sets the specified bit to `0`.
pub fn unset_bit(target: &mut u64, pos: u8) {
	*target &= !bit_mask(pos);
}

pub fn contains_bit(target: u64, pos: u8) -> bool {
	target & bit_mask(pos) > 0
}

/// Constructs a bit mask with only the bit at position `pos` set.
pub fn bit_mask(pos: u8) -> u64 {
	1u64.wrapping_shl(pos as u32)
}

/// Constructs a bit mask that masks out `count` LSBs.
pub fn mask_out_lsb(count: u8) -> u64 {
	u64::MAX.wrapping_shl(count as u32)
}

/// An byte-sized ID allocator that properly reuses free bits.
#[derive(Debug, Clone)]
pub struct U8BitSet([u64; 4]);

impl Default for U8BitSet {
	fn default() -> Self {
		Self::new()
	}
}

impl U8BitSet {
	pub const fn new() -> Self {
		Self([0; 4])
	}

	/// Find the least significant zero bit and sets it to `1`.
	pub fn reserve_zero_bit(&mut self) -> Option<u8> {
		self.0.iter_mut().enumerate().find_map(|(i, word)| {
			let rel_pos = reserve_zero_bit(word);
			if rel_pos != 64 {
				Some(i as u8 * 64 + rel_pos)
			} else {
				None
			}
		})
	}

	/// Find the least significant one bit and sets it to `0`.
	pub fn reserve_set_bit(&mut self) -> Option<u8> {
		self.0.iter_mut().enumerate().find_map(|(i, word)| {
			let rel_pos = reserve_one_bit(word);
			if rel_pos != 64 {
				Some(i as u8 * 64 + rel_pos)
			} else {
				None
			}
		})
	}

	fn word_of(pos: u8) -> usize {
		(pos >> 6) as usize
	}

	fn bit_of(pos: u8) -> u8 {
		pos & 0b111111
	}

	fn decompose_pos(pos: u8) -> (usize, u8) {
		(Self::word_of(pos), Self::bit_of(pos))
	}

	pub fn set(&mut self, pos: u8) {
		let (word, bit) = Self::decompose_pos(pos);
		set_bit(&mut self.0[word], bit)
	}

	pub fn unset(&mut self, pos: u8) {
		let (word, bit) = Self::decompose_pos(pos);
		unset_bit(&mut self.0[word], bit)
	}

	pub fn contains(&self, pos: u8) -> bool {
		let (word, bit) = Self::decompose_pos(pos);
		contains_bit(self.0[word], bit)
	}

	pub fn is_empty(&self) -> bool {
		self.0.iter().all(|word| *word == 0)
	}

	pub fn alloc_all(&mut self) {
		for word in &mut self.0 {
			*word = u64::MAX;
		}
	}

	pub fn free_all_geq(&mut self, min: u8) {
		let min_word_idx = (min / 64) as usize;
		self.0[min_word_idx] &= mask_out_lsb(min % 64);

		for word in &mut self.0[(min_word_idx + 1)..] {
			*word = 0;
		}
	}

	pub fn iter_set(&self) -> U8BitSetIter {
		U8BitSetIter {
			target: self.clone(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct U8BitSetIter {
	target: U8BitSet,
}

impl Iterator for U8BitSetIter {
	type Item = u8;

	fn next(&mut self) -> Option<Self::Item> {
		self.target.reserve_set_bit()
	}
}

// === Batch allocator === //

#[derive(Debug, Clone, Default)]
pub struct LocalBatchAllocator {
	id_generator: u64,
	max_id_batch_exclusive: u64,
}

impl LocalBatchAllocator {
	pub fn generate(&mut self, gen: &AtomicU64, max_id_exclusive: u64, batch_size: u64) -> u64 {
		assert!(batch_size > 0);

		self.id_generator += 1;

		if self.id_generator < self.max_id_batch_exclusive {
			// Fast path
			self.id_generator
		} else {
			self.generate_slow(gen, max_id_exclusive, batch_size)
		}
	}

	#[cold]
	fn generate_slow(&mut self, gen: &AtomicU64, max_id_exclusive: u64, batch_size: u64) -> u64 {
		let start_id = gen
			.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |f| {
				Some(f.saturating_add(batch_size))
			})
			.unwrap();

		self.id_generator = start_id;
		self.max_id_batch_exclusive = start_id.saturating_add(batch_size).min(max_id_exclusive);

		assert!(
			self.id_generator < self.max_id_batch_exclusive,
			"{}",
			"failed to allocate a new batch of IDs: ran out of IDs"
		);

		self.id_generator
	}
}
