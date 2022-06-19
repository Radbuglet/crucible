use crate::util::number::NonZeroNumExt;
use std::{fmt::Debug, num::NonZeroU64};

/// We combine the generation and lock/session field into one `u64` to reduce memory consumption and
/// ensure that we can check the validity of a `.get()` operation in one comparison.
///
/// ## Format
///
/// - Both lock IDs and session IDs (the meta of this ID) are 8 bits long. `meta` takes the least
///   significant byte of the word.
/// - Session and lock IDs of `255` are treated as sentinel `None` values.
/// - By limiting the lock ID size to 8 bits, we make it really easy to fetch the session owner for
///   a given lock ID (just take the LSB of the ID and use it to index into an array with 256 bytes)
///   at the expense of limiting the granularity of our locks.
/// - By keeping the session IDs the same size as our lock ID, we can define the bytes comprising
///   a [SessionLocks] collection as being XOR masks from the associated lock+gen ID to the
///   associated `ONE+gen` ID, which we can then directly compare against the [ExtendedGen]
///   present in the [Obj](super::Obj).
///
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ExtendedGen(u64);

impl Debug for ExtendedGen {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ExtendedGen")
			.field("gen", &self.gen())
			.field("meta", &self.meta())
			.finish()
	}
}

impl ExtendedGen {
	pub fn new(meta: u8, gen: Option<NonZeroU64>) -> Self {
		debug_assert!(gen.prim() < 2u64.pow(64 - 8));

		Self(meta as u64 + (gen.prim() << 8))
	}

	pub fn raw(&self) -> u64 {
		self.0
	}

	pub fn from_raw(id: u64) -> Self {
		Self(id)
	}

	pub fn gen(&self) -> u64 {
		self.0 >> 8
	}

	pub fn meta(&self) -> u8 {
		self.0 as u8
	}

	pub fn xor_meta(self, wrt: u8) -> Self {
		Self(self.0 ^ (wrt as u64))
	}
}

/// See item documentation for [ExtendedGen].
pub struct SessionLocks {
	/// Each lock ID gets its own slot in this array. If the ID is acquired by this session, XOR'ing
	/// the slot's metadata (i.e. the lock ID) with the byte should yield all ones. Otherwise, it
	/// should yield something else. The lock with ID `255` should always be considered acquired.
	lock_states: [u8; 256],
}

impl Default for SessionLocks {
	fn default() -> Self {
		Self {
			// XOR'ing the first `255` lock masks with their lock ID will produce a non-`0xFF` byte.
			// XOR'ing the last lock mask with its lock ID will produce `0xFF`, meaning that it will
			// be automatically acquired.
			lock_states: [0; 256],
		}
	}
}

impl SessionLocks {
	/// Registers the session with ID `sess_id`
	pub fn lock(&mut self, lock_id: u8) {
		debug_assert_ne!(lock_id, 0xFF);

		// (lock_id ^ 0xFF) ^ 0xFF = sess_id
		// lock_id ^ 0xFF = !lock_id
		self.lock_states[lock_id as usize] = !lock_id;
	}

	pub fn check_gen_and_lock(&self, ptr_gen: ExtendedGen, slot_gen: ExtendedGen) -> bool {
		debug_assert_eq!(ptr_gen.meta(), 0xFF);

		let lock_mask = self.lock_states[slot_gen.meta() as usize];
		let slot_gen = slot_gen.xor_meta(lock_mask);
		slot_gen == ptr_gen
	}

	pub fn check_lock(&self, lock: u8) -> bool {
		self.lock_states[lock as usize] ^ lock == 0xFF
	}
}
