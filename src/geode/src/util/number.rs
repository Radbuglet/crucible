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

// === Free bit-list utilities === //

/// Reserves the least significant zero bit from the `target`, marks it as a `1`, and returns its
/// index from the LSB.
///
/// Returns `64` if no bits could be allocated.
pub fn reserve_bit(target: &mut u64) -> u8 {
	let pos = target.trailing_ones() as u8;
	*target |= bit_mask(pos);
	pos
}

/// Sets the specified bit to `0`, marking it as free. `free_bit` uses the same indexing convention
/// as [reserve_bit], i.e. its offset from the LSB. Indices greater than 63 are ignored.
pub fn free_bit(target: &mut u64, pos: u8) {
	*target &= !bit_mask(pos);
}

/// Constructs a bit mask with only the bit at position `pos` set. `bit_mask` uses the same indexing
/// convention as [reserve_bit], i.e. its offset from the LSB. Indices greater than 63 are ignored.
pub fn bit_mask(pos: u8) -> u64 {
	1u64.wrapping_shl(pos as u32)
}

/// Constructs a bit mask that masks out `count` LSBs.
pub fn mask_out_lsb(count: u8) -> u64 {
	u64::MAX.wrapping_shl(count as u32)
}

/// An byte-sized ID allocator that properly reuses free bits.
pub struct U8Alloc([u64; 4]);

impl Default for U8Alloc {
	fn default() -> Self {
		Self::new()
	}
}

impl U8Alloc {
	pub const fn new() -> Self {
		Self([0; 4])
	}

	pub fn alloc(&mut self) -> u8 {
		self.0
			.iter_mut()
			.enumerate()
			.find_map(|(i, word)| {
				let rel_pos = reserve_bit(word);
				if rel_pos != 64 {
					Some(i as u8 * 64 + rel_pos)
				} else {
					None
				}
			})
			.unwrap_or(255)
	}

	pub fn free(&mut self, pos: u8) {
		free_bit(&mut self.0[(pos >> 6) as usize], pos & 0b111111)
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
