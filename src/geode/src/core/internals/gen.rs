use crucible_core::c_enum::c_enum;
use std::fmt;

/// We combine the generation and lock/session field into one `u64` to reduce memory consumption and
/// to ensure that we can check the validity of a `.get()` operation in one comparison.
///
/// ## Format
///
/// Both lock IDs and session IDs (the meta of this ID) are 8 bits long. `meta` takes the least
/// significant byte of the word.
///
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct LockIdAndMeta(u64);

impl fmt::Debug for LockIdAndMeta {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LockIdAndMeta")
			.field("gen", &self.meta())
			.field("meta", &self.meta())
			.finish()
	}
}

impl LockIdAndMeta {
	pub const MAX_META: u64 = 2u64.pow(64 - 8);

	pub fn new(lock: u8, meta: u64) -> Self {
		debug_assert!(meta < Self::MAX_META);

		Self(lock as u64 + (meta << 8))
	}

	pub fn raw(&self) -> u64 {
		self.0
	}

	pub fn from_raw(id: u64) -> Self {
		Self(id)
	}

	pub fn meta(&self) -> u64 {
		self.0 >> 8
	}

	pub fn lock(&self) -> u8 {
		self.0 as u8
	}

	pub fn xor_lock(self, wrt: u8) -> Self {
		Self(self.0 ^ (wrt as u64))
	}

	pub fn or_lock(self, wrt: u8) -> Self {
		Self(self.0 | (wrt as u64))
	}
}

c_enum! {
	pub enum SessionLockMutability {
		Mut,
		Ref,
	}
}

pub struct SessionLocks {
	/// A lock `L` has been acquired for a session slot `S` if it satisfies the specific equalities:
	///
	/// - To borrow it mutably, it must satisfy: `0xFF ^ S = L`.
	/// - To borrow it immutably, it must satisfy: `L | S = 0xFF`.
	///
	/// There are three cases to handle while generating `S` and `H` for a given lock `L`:
	///
	/// - If `L != 0 or 0xFF`,
	///   - Borrowing the slot mutably involves setting `S = !L`.
	///     `forall<L>, 0xFF ^ !L = L  ;  L | !L = 0xFF`
	///
	///   - Borrowing the slot immutably involves setting `S = 0xFF`.
	///     `forall<L> where L != 0, 0xFF ^ 0xFF = 0 != L  ;  L | 0xFF = 0xFF`
	///
	///   - Keeping the slot unborrowed involves setting `S = 0`.
	///     `forall<L> where L != 0xFF, 0xFF ^ 0 = 0xFF != L  ; L | 0 = L != 0xFF`
	///
	/// - If `L = 0`, the slot must always be considered mutably borrowed. `S` can safely be set
	///   to `0xFF` to achieve this behavior:
	///
	///   Mutability: `0xFF ^ 0xFF = 0 = L`
	///   Immutability: `0xFF | 0xFF = 0xFF`
	///
	/// - If `L = 0xFF`, the slot must always be considered immutably borrowed. `S` can safely be set
	///   to `0xFF` to achieve this behavior:
	///
	///   Mutability: `0xFF ^ 0xFF = 0 != L`
	///   Immutability: `0xFF | 0xFF = 0xFF`
	///
	lock_states: [u8; 256],
}

impl Default for SessionLocks {
	fn default() -> Self {
		let mut lock_states = [0; 256];
		lock_states[0] = 0xFF; // Slot `0` is a special case.

		Self { lock_states }
	}
}

impl SessionLocks {
	fn is_regular(id: u8) -> bool {
		id != 0 && id != 0xFF
	}

	pub fn acquire_mut(&mut self, id: u8) {
		debug_assert!(Self::is_regular(id));
		debug_assert_eq!(self.lock_state(id), None);

		self.lock_states[id as usize] = !id;
	}

	pub fn acquire_ref(&mut self, id: u8) {
		debug_assert!(Self::is_regular(id));
		debug_assert_eq!(self.lock_state(id), None);

		self.lock_states[id as usize] = 0xFF;
	}

	pub fn unacquire(&mut self, id: u8) {
		debug_assert!(Self::is_regular(id));
		debug_assert_ne!(self.lock_state(id), None);

		self.lock_states[id as usize] = 0;
	}

	pub fn is_locked_mut(&self, id: u8) -> bool {
		let state = self.lock_states[id as usize];

		0xFF ^ state == id
	}

	pub fn is_locked_ref(&self, id: u8) -> bool {
		let state = self.lock_states[id as usize];

		id | state == 0xFF
	}

	pub fn lock_state(&self, id: u8) -> Option<SessionLockMutability> {
		if self.is_locked_mut(id) {
			debug_assert!(self.is_locked_ref(id));
			Some(SessionLockMutability::Mut)
		} else if self.is_locked_ref(id) {
			Some(SessionLockMutability::Ref)
		} else {
			None
		}
	}

	pub fn is_locked_mut_and_eq(&self, target: LockIdAndMeta, rhs: LockIdAndMeta) -> bool {
		debug_assert_eq!(rhs.lock(), 0xFF);

		let state = self.lock_states[target.lock() as usize];
		rhs.xor_lock(state) == target
	}

	pub fn is_locked_ref_and_eq(&self, target: LockIdAndMeta, rhs: LockIdAndMeta) -> bool {
		debug_assert_eq!(rhs.lock(), 0xFF);

		let state = self.lock_states[target.lock() as usize];
		target.or_lock(state) == rhs
	}
}
