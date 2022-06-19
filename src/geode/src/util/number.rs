use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::mem::replace;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::error::ResultExt;

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

/// Reserves a zero bit from the `target`, marks it as a `1`, and returns its index from the LSB.
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

/// An byte-sized ID allocator that properly reuses free bits.
#[derive(Default)]
pub struct U8Alloc([u64; 4]);

impl U8Alloc {
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

// U64Generator
#[derive(Debug, Clone, Default)]
pub struct U64Generator {
	pub next: u64,
}

impl U64Generator {
	pub const fn new(start_at: u64) -> Self {
		Self { next: start_at }
	}
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

// NonZeroU64Generator
#[derive(Debug, Clone)]
pub struct NonZeroU64Generator {
	pub next: u64,
}

impl NonZeroU64Generator {
	pub const fn new(start_at: u64) -> Self {
		assert!(start_at > 0);
		Self { next: start_at }
	}
}

impl Default for NonZeroU64Generator {
	fn default() -> Self {
		Self::new(1)
	}
}

impl NumberGenMut for NonZeroU64Generator {
	type Value = NonZeroU64;
	type GenError = GenOverflowError<u64>;

	fn try_generate_mut(&mut self) -> Result<Self::Value, Self::GenError> {
		let yielded = NonZeroU64::new(self.next).unwrap();
		self.next = self
			.next
			.checked_add(1)
			.ok_or(GenOverflowError { limit: u64::MAX })?;

		Ok(yielded)
	}
}

// AtomicU64Generator
#[derive(Debug, Default)]
pub struct AtomicU64Generator {
	pub next: AtomicU64,
}

impl AtomicU64Generator {
	pub const fn new(start_at: u64) -> Self {
		Self {
			next: AtomicU64::new(start_at),
		}
	}
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

impl AtomicNZU64Generator {
	pub const fn new(start_at: u64) -> Self {
		assert!(start_at > 0);
		Self {
			next: AtomicU64Generator::new(start_at),
		}
	}
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

	#[inline(never)]
	fn generate_slow(&mut self, gen: &AtomicU64, max_id_exclusive: u64, batch_size: u64) -> u64 {
		const TOO_MANY_IDS_ERROR: &str = "failed to allocate a new batch of IDs: ran out of IDs";

		let start_id = gen
			.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |f| {
				f.checked_add(batch_size)
			})
			.expect(TOO_MANY_IDS_ERROR);

		assert!(start_id < max_id_exclusive, "{}", TOO_MANY_IDS_ERROR);

		self.id_generator = start_id;
		self.max_id_batch_exclusive = start_id.saturating_add(batch_size).min(max_id_exclusive);
		self.id_generator
	}
}
