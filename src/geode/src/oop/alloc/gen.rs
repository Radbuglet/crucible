/// An byte-sized ID allocator that treats `255` as a sentinel value. Used to allocate session and
/// lock IDs.
pub struct IdAlloc([u64; 4]);

impl Default for IdAlloc {
	fn default() -> Self {
		Self([0, 0, 0, bit_mask(62)])
	}
}

impl IdAlloc {
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

/// Reserves a zero bit from the `target`, marks it as a `1`, and returns its index from the LSB.
/// Returns `64` if no bits could be allocated.
fn reserve_bit(target: &mut u64) -> u8 {
	let pos = target.trailing_ones() as u8;
	*target |= bit_mask(pos);
	pos
}

/// Sets the specified bit to `0`, marking it as free. `free_bit` uses the same indexing convention
/// as [reserve_bit], i.e. its offset from the LSB. Indices greater than 63 are ignored.
fn free_bit(target: &mut u64, pos: u8) {
	*target &= !bit_mask(pos);
}

/// Constructs a bit mask with only the bit at position `pos` set. `bit_mask` uses the same indexing
/// convention as [reserve_bit], i.e. its offset from the LSB. Indices greater than 63 are ignored.
fn bit_mask(pos: u8) -> u64 {
	1u64.wrapping_shl(pos as u32)
}

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
///   associated `ONE+gen` ID, which we can then directly compare against the [ExtendedGen]\
///   present in the [Obj].
///
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ExtendedGen(u64);

impl ExtendedGen {
	pub fn new(meta: u8, gen: u64) -> Self {
		debug_assert!(gen < 2u64.pow(64 - 8));

		Self(meta as u64 + gen << 8)
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

	pub fn can_lock(&self, ptr_gen: ExtendedGen, slot_gen: ExtendedGen) -> bool {
		debug_assert_eq!(ptr_gen.meta(), 0xFF);

		let lock_mask = self.lock_states[slot_gen.meta() as usize];
		let slot_gen = slot_gen.xor_meta(lock_mask);
		slot_gen == ptr_gen
	}
}
