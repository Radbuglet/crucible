use std::{error::Error, fmt, num::NonZeroUsize};

use crucible_core::{c_enum::c_enum, error::ResultExt};
use thiserror::Error;

use self::math::UserLockSetIter;

use super::{
	debug::DebugLabel,
	owned::{Destructible, Owned},
	session::Session,
};

// === Math === //

mod math {
	use std::{
		borrow::Cow,
		cell::Cell,
		fmt,
		num::{NonZeroU8, NonZeroUsize},
		ops::{Index, IndexMut},
	};

	use crucible_core::array::arr;

	use crate::{
		core::debug::SerializedDebugLabel,
		util::number::{bit_mask, U8BitSet},
	};

	use super::{BorrowError, BorrowMutability, BorrowState};

	// === Lock Ids === //

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub struct LockId(u8);

	impl LockId {
		pub const TOTAL_ID_COUNT: usize = 256;
		pub const MUTABLE: LockId = LockId(0);
		pub const IMMUTABLE: LockId = LockId(0xFF);

		pub fn from_user_id(id: UserLockId) -> Self {
			Self(id.0.get())
		}

		pub const fn from_index(index: usize) -> Self {
			assert!(
				index < Self::TOTAL_ID_COUNT,
				"`index` is greater than or equal to `LockId::ID_COUNT`"
			);
			Self(index as u8)
		}

		pub fn try_as_user_id(self) -> Option<UserLockId> {
			UserLockId::try_from_lock_id(self)
		}

		pub fn is_user_lock(self) -> bool {
			self.try_as_user_id().is_some()
		}

		pub const fn default_debug_label(self) -> SerializedDebugLabel {
			match self.0 {
				0 => Some(Cow::Borrowed("[always mutable]")),
				1..=0xFE => None,
				0xFF => Some(Cow::Borrowed("[always immutable]")),
			}
		}

		fn bit_index(self) -> u8 {
			self.0
		}

		fn index(self) -> usize {
			self.0 as usize
		}
	}

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub struct UserLockId(NonZeroU8);

	impl UserLockId {
		pub const USER_ID_COUNT: usize = 256 - 2;

		fn try_new(id: u8) -> Option<Self> {
			if id != 0 && id != 0xFF {
				Some(Self(NonZeroU8::new(id).unwrap()))
			} else {
				None
			}
		}

		pub fn try_from_lock_id(id: LockId) -> Option<Self> {
			Self::try_new(id.0)
		}

		pub fn as_lock_id(self) -> LockId {
			LockId::from_user_id(self)
		}

		fn bit_index(self) -> u8 {
			self.0.get()
		}

		fn index(self) -> usize {
			self.bit_index() as usize
		}
	}

	#[derive(Copy, Clone, Hash, Eq, PartialEq)]
	pub struct LockIdAndMeta(u64);

	impl fmt::Debug for LockIdAndMeta {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("LockIdAndMeta")
				.field("lock", &self.lock())
				.field("meta", &self.meta())
				.finish()
		}
	}

	impl LockIdAndMeta {
		pub const MAX_META_EXCLUSIVE: u64 = 2u64.pow(64 - 8);

		const fn new_raw(lock: u8, meta: u64) -> Self {
			debug_assert!(meta < Self::MAX_META_EXCLUSIVE);

			Self(lock as u64 + (meta << 8))
		}

		pub const fn new(lock: LockId, meta: u64) -> Self {
			Self::new_raw(lock.0, meta)
		}

		pub const fn new_handle(meta: u64) -> Self {
			Self::new_raw(0xFF, meta)
		}

		pub fn could_be_a_handle(self) -> bool {
			self.lock_raw() == 0xFF
		}

		pub fn turn_into_a_handle(self) -> Self {
			self.or_lock(0xFF)
		}

		pub fn raw(self) -> u64 {
			self.0
		}

		pub fn from_raw(id: u64) -> Self {
			Self(id)
		}

		pub fn meta(self) -> u64 {
			self.0 >> 8
		}

		fn lock_raw(self) -> u8 {
			self.0 as u8
		}

		pub fn lock(self) -> LockId {
			LockId(self.lock_raw())
		}

		fn xor_lock(self, wrt: u8) -> Self {
			Self(self.0 ^ (wrt as u64))
		}

		fn or_lock(self, wrt: u8) -> Self {
			Self(self.0 | (wrt as u64))
		}
	}

	// === SessionLockStateTracker === //

	pub struct SessionLockStateTracker {
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
		lock_states: [Cell<u8>; 256],
	}

	impl Default for SessionLockStateTracker {
		fn default() -> Self {
			let lock_states = arr![Cell::new(0); 256];
			lock_states[0].set(0xFF); // Slot `0` is a special case.

			Self { lock_states }
		}
	}

	impl SessionLockStateTracker {
		pub fn acquire(&self, id: UserLockId, mutability: BorrowMutability) {
			match mutability {
				BorrowMutability::Ref => self.acquire_ref(id),
				BorrowMutability::Mut => self.acquire_mut(id),
			}
		}

		pub fn acquire_mut(&self, id: UserLockId) {
			debug_assert_eq!(self.lock_state(id.as_lock_id()), None);

			self.lock_states[id.index()].set(!id.bit_index());
		}

		pub fn acquire_ref(&self, id: UserLockId) {
			debug_assert_eq!(self.lock_state(id.as_lock_id()), None);

			self.lock_states[id.index()].set(0xFF);
		}

		pub fn unacquire(&self, id: UserLockId) {
			debug_assert_ne!(self.lock_state(id.as_lock_id()), None);

			self.lock_states[id.index()].set(0);
		}

		pub fn is_locked_mut(&self, id: LockId) -> bool {
			let state = self.lock_states[id.index()].get();

			0xFF ^ state == id.bit_index()
		}

		pub fn is_locked_ref(&self, id: LockId) -> bool {
			let state = self.lock_states[id.index()].get();

			id.bit_index() | state == 0xFF
		}

		pub fn lock_state(&self, id: LockId) -> Option<BorrowMutability> {
			if self.is_locked_mut(id) {
				debug_assert!(self.is_locked_ref(id));
				Some(BorrowMutability::Mut)
			} else if self.is_locked_ref(id) {
				Some(BorrowMutability::Ref)
			} else {
				None
			}
		}

		pub fn is_locked_mut_and_eq(&self, target: LockIdAndMeta, handle: LockIdAndMeta) -> bool {
			debug_assert!(handle.could_be_a_handle());

			let state = self.lock_states[target.lock().index()].get();
			handle.xor_lock(state) == target
		}

		pub fn is_locked_ref_and_eq(&self, target: LockIdAndMeta, handle: LockIdAndMeta) -> bool {
			debug_assert!(handle.could_be_a_handle());

			let state = self.lock_states[target.lock().index()].get();
			target.or_lock(state) == handle
		}
	}

	// === LockMap === //

	#[derive(Debug, Clone)]
	pub struct LockMap<T> {
		values: [T; 256],
	}

	impl<T> LockMap<T> {
		pub const fn new(values: [T; LockId::TOTAL_ID_COUNT]) -> Self {
			Self { values }
		}
	}

	impl<T> Index<LockId> for LockMap<T> {
		type Output = T;

		fn index(&self, index: LockId) -> &Self::Output {
			&self.values[index.index()]
		}
	}

	impl<T> IndexMut<LockId> for LockMap<T> {
		fn index_mut(&mut self, index: LockId) -> &mut Self::Output {
			&mut self.values[index.index()]
		}
	}

	// === UserLockSet === //

	#[derive(Debug, Clone, Copy, Default)]
	pub struct UserLockSet(U8BitSet);

	impl UserLockSet {
		pub fn add(&mut self, id: UserLockId) {
			self.0.set(id.bit_index());
		}

		pub fn clear(&mut self) {
			self.0.clear();
		}

		pub fn iter(&self) -> UserLockSetIter {
			UserLockSetIter(self.0)
		}

		pub fn drain(&mut self) -> UserLockSetIter {
			let iter = self.iter();
			self.clear();
			iter
		}
	}

	#[derive(Debug, Clone)]
	pub struct UserLockSetIter(U8BitSet);

	impl Iterator for UserLockSetIter {
		type Item = UserLockId;

		fn next(&mut self) -> Option<Self::Item> {
			let bit = self.0.reserve_set_bit()?;
			Some(UserLockId::try_new(bit).unwrap())
		}
	}

	// === UserLockIdAllocator === //

	#[derive(Debug, Clone)]
	pub struct UserLockIdAllocator(U8BitSet);

	impl UserLockIdAllocator {
		pub const fn new() -> Self {
			Self(U8BitSet([bit_mask(0), 0, 0, bit_mask(63)]))
		}

		pub fn reserve(&mut self, at_most: usize) -> UserLockSetIter {
			let mut reserved = U8BitSet::new();

			for _ in 0..at_most {
				let bit = match self.0.reserve_zero_bit() {
					Some(bit) => bit,
					None => break,
				};
				reserved.set(bit);
			}

			UserLockSetIter(reserved)
		}

		pub fn unreserve(&mut self, id: UserLockId) {
			self.0.unset(id.bit_index())
		}
	}

	// === LockBorrowCount === //

	#[derive(Debug, Clone)]
	pub struct LockBorrowCount(i16);

	impl LockBorrowCount {
		pub const fn new() -> Self {
			Self(0)
		}

		pub fn borrow_state(&self) -> Option<BorrowState> {
			match self.0 {
				-1 => Some(BorrowState::Mut),
				0 => None,
				1..=i16::MAX => Some(BorrowState::Ref(
					NonZeroUsize::new(self.0 as usize).unwrap(),
				)),
				_ => unreachable!(),
			}
		}

		pub fn can_acquire(&self, mutability: BorrowMutability) -> Result<(), BorrowError> {
			let is_compatible = match mutability {
				BorrowMutability::Mut => self.0 == 0,
				BorrowMutability::Ref => self.0 >= 0,
			};

			if is_compatible {
				Ok(())
			} else {
				Err(BorrowError {
					offending_state: self.borrow_state().unwrap(),
				})
			}
		}

		pub fn acquire(&mut self, mutability: BorrowMutability) {
			debug_assert!(self.can_acquire(mutability).is_ok());

			match mutability {
				BorrowMutability::Ref => {
					// Safety: will not overflow; provided by caller
					self.0 += 1;
				}
				BorrowMutability::Mut => {
					self.0 = -1;
				}
			}
		}

		pub fn unacquire(&mut self) {
			match self.0 {
				-1 => self.0 = 0,
				1..=i16::MAX => self.0 -= 1,
				_ => unreachable!(),
			}
		}
	}
}

// === Global State === //

mod db {
	use std::{cell::Cell, fmt};

	use crucible_core::array::{arr, arr_indexed};
	use parking_lot::Mutex;

	use crate::{
		core::{
			debug::SerializedDebugLabel,
			session::{Session, StaticStorageGetter, StaticStorageHandler},
		},
		util::threading::new_lot_mutex,
	};

	use super::{
		math::{
			LockBorrowCount, LockId, LockIdAndMeta, LockMap, SessionLockStateTracker, UserLockId,
			UserLockIdAllocator, UserLockSet, UserLockSetIter,
		},
		BorrowMutability, BorrowState, SessionLocksAcquisitionError, UserLock,
	};

	// === Global State === //

	struct GlobalLockState {
		/// The set of all locks logically reserved by the user.
		reserved_locks: UserLockIdAllocator,

		/// Borrow counts of every cell.
		lock_rcs: LockMap<LockBorrowCount>,

		/// Debug labels for each lock slot.
		lock_labels: LockMap<SerializedDebugLabel>,
	}

	static GLOBAL_LOCK_STATE: Mutex<GlobalLockState> = new_lot_mutex(GlobalLockState {
		reserved_locks: UserLockIdAllocator::new(),
		lock_rcs: LockMap::new(arr![LockBorrowCount::new(); LockId::TOTAL_ID_COUNT]),
		lock_labels: LockMap::new(
			arr_indexed![i => LockId::from_index(i).default_debug_label(); LockId::TOTAL_ID_COUNT],
		),
	});

	#[derive(Default)]
	pub(crate) struct SessionStateLockManager {
		/// A container storing the states of every slot.
		lock_states: SessionLockStateTracker,

		/// A bitset of all slots that require zeroing and unlocking in the global
		requires_unlocking: Cell<UserLockSet>,
	}

	impl StaticStorageHandler for SessionStateLockManager {
		type Comp = Self;

		fn init_comp(target: &mut Option<Self::Comp>) {
			if target.is_none() {
				*target = Some(Self::default());
			}
		}

		fn deinit_comp(target: &mut Option<Self::Comp>) {
			let mut global = GLOBAL_LOCK_STATE.lock();
			let target = target.as_mut().unwrap();

			// Unlock all acquired locks
			for slot in target.requires_unlocking.get_mut().drain() {
				// Unlock locally
				target.lock_states.unacquire(slot);

				// Unlock globally
				global.lock_rcs[slot.as_lock_id()].unacquire();
			}
		}
	}

	// === Lock reservation management === //

	pub fn reserve_lock(debug_name: SerializedDebugLabel) -> UserLockId {
		let mut global = GLOBAL_LOCK_STATE.lock();

		let id = global.reserved_locks.reserve(1).next().unwrap_or_else(|| {
			panic!(
				"cannot allocate more than {} locks concurrently",
				UserLockId::USER_ID_COUNT
			)
		});

		global.lock_labels[id.as_lock_id()] = debug_name;
		id
	}

	pub fn reserve_many_locks(at_most: usize) -> UserLockSetIter {
		GLOBAL_LOCK_STATE.lock().reserved_locks.reserve(at_most)
	}

	pub fn unreserve_lock(id: UserLockId) {
		let mut state = GLOBAL_LOCK_STATE.lock();

		state.reserved_locks.unreserve(id);
		state.lock_labels[id.as_lock_id()] = None;
	}

	pub fn set_lock_debug_name(id: UserLockId, name: SerializedDebugLabel) {
		GLOBAL_LOCK_STATE.lock().lock_labels[id.as_lock_id()] = name;
	}

	#[derive(Clone)]
	struct InternalLockDebugNameFormatter<'a> {
		global: &'a GlobalLockState,
		lock_id: LockId,
	}

	impl fmt::Display for InternalLockDebugNameFormatter<'_> {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			if let Some(label) = &self.global.lock_labels[self.lock_id] {
				f.write_str(label)
			} else {
				f.write_str("[unspecified]")
			}
		}
	}

	#[derive(Debug, Clone)]
	pub struct LockDebugNameFormatter(LockId);

	impl fmt::Display for LockDebugNameFormatter {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			InternalLockDebugNameFormatter {
				global: &mut GLOBAL_LOCK_STATE.lock(),
				lock_id: self.0,
			}
			.fmt(f)
		}
	}

	pub fn get_lock_debug_name(id: LockId) -> LockDebugNameFormatter {
		LockDebugNameFormatter(id)
	}

	pub fn acquire_locks(
		s: Session,
		list: &[(BorrowMutability, UserLockId)],
	) -> Result<(), SessionLocksAcquisitionError> {
		let mut global = GLOBAL_LOCK_STATE.lock();
		let state = SessionStateLockManager::get(s);

		// Ensure that we can acquire these locks.
		{
			let mut violations = Vec::new();

			for (mutability, id) in list.iter().copied() {
				// Ensure that we haven't already acquired this lock.
				if state.lock_states.is_locked_ref(id.as_lock_id()) {
					// This lock has already been acquired by this session
					continue;
				}

				if let Err(err) = global.lock_rcs[id.as_lock_id()].can_acquire(mutability) {
					violations.push((UserLock(id), err))
				}
			}

			if !violations.is_empty() {
				return Err(SessionLocksAcquisitionError { violations });
			}
		}

		// Acquire those locks.
		{
			let mut requires_unlocking = state.requires_unlocking.get();

			for (mutability, id) in list.iter().copied() {
				global.lock_rcs[id.as_lock_id()].acquire(mutability);
				state.lock_states.acquire(id, mutability);
				requires_unlocking.add(id);
			}

			state.requires_unlocking.set(requires_unlocking);
		}

		Ok(())
	}

	// === Lock state querying === //

	pub fn get_global_lock_borrow_state(id: UserLockId) -> Option<BorrowState> {
		GLOBAL_LOCK_STATE.lock().lock_rcs[id.as_lock_id()].borrow_state()
	}

	pub fn get_session_borrow_state(session: Session, id: LockId) -> Option<BorrowMutability> {
		SessionStateLockManager::get(session)
			.lock_states
			.lock_state(id)
	}

	pub fn extended_eq_locked_mut(
		session: Session,
		target: LockIdAndMeta,
		handle: LockIdAndMeta,
	) -> bool {
		SessionStateLockManager::get(session)
			.lock_states
			.is_locked_mut_and_eq(target, handle)
	}

	pub fn extended_eq_locked_ref(
		session: Session,
		target: LockIdAndMeta,
		handle: LockIdAndMeta,
	) -> bool {
		SessionStateLockManager::get(session)
			.lock_states
			.is_locked_ref_and_eq(target, handle)
	}
}

pub(crate) use db::SessionStateLockManager;

// === Generic lock state === //

c_enum! {
	pub enum BorrowMutability {
		Ref,
		Mut,
	}
}

impl BorrowMutability {
	pub fn inverse(self) -> Self {
		match self {
			BorrowMutability::Ref => BorrowMutability::Mut,
			BorrowMutability::Mut => BorrowMutability::Ref,
		}
	}

	fn fmt_adjective(self) -> &'static str {
		match self {
			BorrowMutability::Ref => "immutably",
			BorrowMutability::Mut => "mutably",
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum BorrowState {
	Ref(NonZeroUsize),
	Mut,
}

impl BorrowState {
	pub fn mutability(self) -> BorrowMutability {
		match self {
			BorrowState::Ref(_) => BorrowMutability::Ref,
			BorrowState::Mut => BorrowMutability::Mut,
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct BorrowError {
	pub offending_state: BorrowState,
}

impl Error for BorrowError {}

impl fmt::Display for BorrowError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.offending_state {
			BorrowState::Ref(blocked_by) => write!(
				f,
				"failed to acquire lock mutably: blocked by {blocked_by} immutable lock{}.",
				if blocked_by.get() == 1 { "" } else { "s" }
			),
			BorrowState::Mut => {
				f.write_str("failed to acquire lock immutably: blocked by 1 mutable lock.")
			}
		}
	}
}

// === UserLock === //

#[derive(Debug, Clone, Error)]
pub struct SessionLocksAcquisitionError {
	violations: Vec<(UserLock, BorrowError)>,
}

impl fmt::Display for SessionLocksAcquisitionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "Failed to acquire session locks:\n")?;

		for (lock, error) in self.violations.iter() {
			writeln!(f, "- {:?}: {}", lock, error)?;
		}

		Ok(())
	}
}

#[derive(Debug, Copy, Clone, Error)]
pub struct LockPermissionsError {
	pub lock: Lock,
	pub requested_mode: BorrowMutability,
	pub session_had_lock: bool,
}

impl fmt::Display for LockPermissionsError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"failed to borrow {:?} {}",
			self.lock,
			self.requested_mode.fmt_adjective(),
		)?;

		if self.session_had_lock {
			write!(
				f,
				"; session only acquired the lock {}",
				self.requested_mode.inverse().fmt_adjective(),
			)?;
		} else {
			write!(f, "; session did not acquire the lock at all.")?;
		}

		Ok(())
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct UserLock(math::UserLockId);

impl fmt::Debug for UserLock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("UserLock")
			.field("id", &self.0)
			.field("debug_name", &db::get_lock_debug_name(self.0.as_lock_id()))
			.finish()
	}
}

impl UserLock {
	pub const USER_ID_COUNT: usize = math::UserLockId::USER_ID_COUNT;

	pub fn new<L: DebugLabel>(label: L) -> Owned<Self> {
		let id = db::reserve_lock(label.to_debug_label());
		Owned::new(UserLock(id))
	}

	pub fn new_batch(at_most: usize) -> UserLockAllocIter {
		UserLockAllocIter(db::reserve_many_locks(at_most))
	}

	pub fn try_from_lock(lock: Lock) -> Option<Self> {
		lock.0.try_as_user_id().map(Self)
	}

	pub fn set_debug_name<L: DebugLabel>(self, label: L) {
		db::set_lock_debug_name(self.0, label.to_debug_label())
	}

	pub fn global_borrow_state(self) -> Option<BorrowState> {
		db::get_global_lock_borrow_state(self.0)
	}

	pub fn as_lock(self) -> Lock {
		Lock::from_user_lock(self)
	}
}

impl Destructible for UserLock {
	fn destruct(self) {
		db::unreserve_lock(self.0)
	}
}

impl Session<'_> {
	pub fn try_acquire_locks<I: IntoIterator<Item = (BorrowMutability, UserLock)>>(
		self,
		locks: I,
	) -> Result<(), SessionLocksAcquisitionError> {
		db::acquire_locks(
			self,
			&locks
				.into_iter()
				.map(|(mutability, id)| (mutability, id.0))
				.collect::<Vec<_>>(),
		)
	}

	pub fn acquire_locks<I: IntoIterator<Item = (BorrowMutability, UserLock)>>(self, locks: I) {
		self.try_acquire_locks(locks).unwrap_pretty()
	}
}

#[derive(Debug)]
pub struct UserLockAllocIter(UserLockSetIter);

impl Iterator for UserLockAllocIter {
	type Item = Owned<UserLock>;

	fn next(&mut self) -> Option<Self::Item> {
		let id = self.0.next()?;
		Some(Owned::new(UserLock(id)))
	}
}

impl Drop for UserLockAllocIter {
	fn drop(&mut self) {
		for item in self {
			drop(item); // Destructs the `Owned` lock instance.
		}
	}
}

// === Lock === //

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct Lock(math::LockId);

impl fmt::Debug for Lock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lock")
			.field("id", &self.0)
			.field("debug_name", &db::get_lock_debug_name(self.0))
			.finish()
	}
}

impl Lock {
	pub const TOTAL_ID_COUNT: usize = math::LockId::TOTAL_ID_COUNT;
	pub const IMMUTABLE: Self = Lock(math::LockId::IMMUTABLE);
	pub const MUTABLE: Self = Lock(math::LockId::MUTABLE);

	pub fn from_user_lock(lock: UserLock) -> Self {
		Self(lock.0.as_lock_id())
	}

	pub fn try_as_user_lock(self) -> Option<UserLock> {
		UserLock::try_from_lock(self)
	}

	pub fn session_borrow_state(self, session: Session) -> Option<BorrowMutability> {
		db::get_session_borrow_state(session, self.0)
	}

	pub fn is_user_lock(&self) -> bool {
		self.0.is_user_lock()
	}
}

// === LockAndMeta === //

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct LockAndMeta(math::LockIdAndMeta);

impl fmt::Debug for LockAndMeta {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LockAndMeta")
			.field("lock", &self.lock())
			.field("meta", &self.0.meta())
			.finish()
	}
}

impl LockAndMeta {
	pub const MAX_META_EXCLUSIVE: u64 = math::LockIdAndMeta::MAX_META_EXCLUSIVE;

	pub fn new(lock: Lock, meta: u64) -> Self {
		Self(math::LockIdAndMeta::new(lock.0, meta))
	}

	pub fn new_handle(meta: u64) -> Self {
		Self(math::LockIdAndMeta::new_handle(meta))
	}

	pub fn from_raw(raw: u64) -> Self {
		Self(math::LockIdAndMeta::from_raw(raw))
	}

	pub fn raw(self) -> u64 {
		self.0.raw()
	}

	pub fn lock(self) -> Lock {
		Lock(self.0.lock())
	}

	pub fn meta(self) -> u64 {
		self.0.meta()
	}

	pub fn could_be_a_handle(self) -> bool {
		self.0.could_be_a_handle()
	}

	pub fn make_handle(self) -> Self {
		LockAndMeta(self.0.turn_into_a_handle())
	}

	pub fn eq_locked(self, session: Session, handle: Self, mutability: BorrowMutability) -> bool {
		match mutability {
			BorrowMutability::Ref => self.eq_locked_ref(session, handle),
			BorrowMutability::Mut => self.eq_locked_mut(session, handle),
		}
	}

	pub fn eq_locked_mut(self, session: Session, handle: Self) -> bool {
		db::extended_eq_locked_mut(session, self.0, handle.0)
	}

	pub fn eq_locked_ref(self, session: Session, handle: Self) -> bool {
		db::extended_eq_locked_ref(session, self.0, handle.0)
	}
}
