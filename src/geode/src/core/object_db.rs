use std::fmt;

use super::{
	lock::{BorrowMutability, Lock, LockAndMeta, LockPermissionsError},
	session::Session,
};

// === Core === //

mod db {
	use crucible_core::cell::UnsafeCellExt;

	use crate::{
		core::{
			lock::{BorrowMutability, Lock, LockAndMeta},
			session::{Session, StaticStorageGetter, StaticStorageHandler},
		},
		util::{bump::LeakyBump, number::LocalBatchAllocator},
	};
	use std::{
		cell::UnsafeCell,
		fmt,
		sync::atomic::{AtomicPtr, AtomicU64, Ordering as AtomicOrdering},
	};

	const ID_GEN_BATCH_SIZE: u64 = 4096 * 4096;

	static GLOBAL_GEN_ALLOC: AtomicU64 = AtomicU64::new(0);

	#[derive(Default)]
	pub(crate) struct SessionSlotManagerState {
		free_slots: Vec<&'static Slot>,
		slot_alloc: LeakyBump,
		gen_alloc: LocalBatchAllocator,
	}

	impl StaticStorageHandler for SessionSlotManagerState {
		type Comp = UnsafeCell<Self>;

		fn init_comp(target: &mut Option<Self::Comp>) {
			if target.is_none() {
				*target = Some(Default::default());
			}
		}
	}

	#[derive(Default)]
	pub struct Slot {
		gen: AtomicU64,
		ptr: AtomicPtr<()>,
	}

	impl fmt::Debug for Slot {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			f.debug_struct("Slot")
				.field(
					"gen",
					&LockAndMeta::from_raw(self.gen.load(AtomicOrdering::Relaxed)),
				)
				.field("ptr", &self.ptr)
				.finish()
		}
	}

	impl Slot {
		/// Updates the slot's target pointer to `base_ptr` and updates the slot's generation and
		/// lock.
		///
		/// ## Safety
		///
		/// This operation can only be performed by one thread at a time and the generation provided
		/// by `lock_and_gen` must be monotonically increasing for this one [Slot].
		///
		fn acquire(&self, lock_and_gen: LockAndMeta, base_ptr: *mut ()) {
			// We reserve generation `0` for a sentinel unacquired value.
			debug_assert_ne!(lock_and_gen.meta(), 0);

			self.gen.store(lock_and_gen.raw(), AtomicOrdering::Relaxed);
			self.ptr.store(base_ptr, AtomicOrdering::Release);
		}

		/// Clears the slot if its generation matches the generation provided in `gen_handle`, returning
		/// an [Err] with the current slot's [LockAndMeta].
		///
		/// ## Safety
		///
		/// Can be called on as many threads as desired.
		///
		fn try_destroy(&self, gen_handle: LockAndMeta) -> Result<(), LockAndMeta> {
			debug_assert!(gen_handle.could_be_a_handle());

			let replaced_gen = self.gen.load(AtomicOrdering::Relaxed);
			let replaced_gen = LockAndMeta::from_raw(replaced_gen);

			if replaced_gen.make_handle() == gen_handle {
				self.gen.store(0, AtomicOrdering::Relaxed);
				Ok(())
			} else {
				Err(replaced_gen)
			}
		}

		pub fn try_fetch(
			&self,
			session: Session,
			gen_handle: LockAndMeta,
			mutability: BorrowMutability,
		) -> Result<*mut (), LockAndMeta> {
			let ptr = self.ptr.load(AtomicOrdering::Acquire);
			let gen = self.gen.load(AtomicOrdering::Relaxed);
			let gen = LockAndMeta::from_raw(gen);

			if gen.eq_locked(session, gen_handle, mutability) {
				Ok(ptr)
			} else {
				Err(gen)
			}
		}

		pub fn try_fetch_no_lock(&self, gen_handle: LockAndMeta) -> Result<*mut (), LockAndMeta> {
			let ptr = self.ptr.load(AtomicOrdering::Acquire);
			let gen = self.gen.load(AtomicOrdering::Relaxed);
			let gen = LockAndMeta::from_raw(gen);

			debug_assert!(gen.could_be_a_handle());

			if gen == gen_handle {
				Ok(ptr)
			} else {
				Err(gen)
			}
		}

		pub fn fetch_unchecked(&self) -> *mut () {
			self.ptr.load(AtomicOrdering::Relaxed)
		}
	}

	pub fn acquire_slot(
		session: Session,
		lock: Lock,
		base_ptr: *mut (),
	) -> (&'static Slot, LockAndMeta) {
		let state = unsafe { SessionSlotManagerState::get(session).get_mut_unchecked() };

		// Allocate generation
		let gen = state.gen_alloc.generate(
			&GLOBAL_GEN_ALLOC,
			LockAndMeta::MAX_META_EXCLUSIVE,
			ID_GEN_BATCH_SIZE,
		);
		let lock = LockAndMeta::new(lock, gen);

		#[cold]
		#[inline(never)]
		fn allocate_new_slot(state: &mut SessionSlotManagerState) -> &'static Slot {
			state.slot_alloc.alloc(Slot::default())
		}

		let slot = if let Some(slot) = state.free_slots.pop() {
			slot
		} else {
			allocate_new_slot(state)
		};

		slot.acquire(lock, base_ptr);
		(slot, lock.make_handle())
	}

	pub fn destroy_slot(
		session: Session,
		slot: &'static Slot,
		gen_handle: LockAndMeta,
	) -> Result<(), LockAndMeta> {
		let state = unsafe { SessionSlotManagerState::get(session).get_mut_unchecked() };

		match slot.try_destroy(gen_handle) {
			Ok(_) => {
				state.free_slots.push(slot);
				Ok(())
			}
			Err(err) => Err(err),
		}
	}
}

use crucible_core::error::ErrorFormatExt;
pub(crate) use db::SessionSlotManagerState;
use thiserror::Error;

// === User interface === //

#[derive(Debug, Copy, Clone)]
pub struct Slot(&'static db::Slot);

impl Slot {
	pub fn new(session: Session, lock: Lock, base_ptr: *mut ()) -> (Self, LockAndMeta) {
		let (slot, handle) = db::acquire_slot(session, lock, base_ptr);

		(Slot(slot), handle)
	}

	pub fn try_destroy(
		self,
		session: Session,
		gen_handle: LockAndMeta,
	) -> Result<(), SlotDeadError> {
		match db::destroy_slot(session, self.0, gen_handle) {
			Ok(_) => Ok(()),
			Err(received_handle) => Err(SlotDeadError {
				requested_handle: gen_handle,
				offending_descriptor: received_handle,
			}),
		}
	}

	pub fn try_fetch(
		self,
		session: Session,
		gen_handle: LockAndMeta,
		mutability: BorrowMutability,
	) -> Result<*mut (), SlotAccessError> {
		self.0
			.try_fetch(session, gen_handle, mutability)
			.map_err(|offending_descriptor| {
				SlotAccessError::decode(session, mutability, gen_handle, offending_descriptor)
			})
	}

	pub fn fetch(
		self,
		session: Session,
		gen_handle: LockAndMeta,
		mutability: BorrowMutability,
	) -> *mut () {
		#[cold]
		#[inline(never)]
		fn process_error(
			session: Session,
			requested_mutability: BorrowMutability,
			requested_handle: LockAndMeta,
			offending_descriptor: LockAndMeta,
		) -> ! {
			SlotAccessError::decode(
				session,
				requested_mutability,
				requested_handle,
				offending_descriptor,
			)
			.raise();
		}

		match self.0.try_fetch(session, gen_handle, mutability) {
			Ok(ptr) => ptr,
			Err(offending_descriptor) => {
				process_error(session, mutability, gen_handle, offending_descriptor)
			}
		}
	}

	pub fn try_fetch_no_lock(self, gen_handle: LockAndMeta) -> Result<*mut (), SlotDeadError> {
		self.0
			.try_fetch_no_lock(gen_handle)
			.map_err(|offending_descriptor| SlotDeadError {
				offending_descriptor,
				requested_handle: gen_handle,
			})
	}

	pub fn fetch_no_lock(self, gen_handle: LockAndMeta) -> *mut () {
		#[cold]
		#[inline(never)]
		fn process_error(requested_handle: LockAndMeta, offending_descriptor: LockAndMeta) -> ! {
			SlotDeadError {
				offending_descriptor,
				requested_handle,
			}
			.raise()
		}

		match self.0.try_fetch_no_lock(gen_handle) {
			Ok(ptr) => ptr,
			Err(offending_descriptor) => process_error(gen_handle, offending_descriptor),
		}
	}

	pub fn fetch_unchecked(self) -> *mut () {
		self.0.fetch_unchecked()
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to fetch `Slot`")]
pub enum SlotAccessError {
	Dead(#[from] SlotDeadError),
	Locked(#[from] LockPermissionsError),
}

impl SlotAccessError {
	fn decode(
		session: Session,
		requested_mutability: BorrowMutability,
		requested_handle: LockAndMeta,
		offending_descriptor: LockAndMeta,
	) -> Self {
		if requested_handle.meta() != offending_descriptor.meta() {
			Self::Dead(SlotDeadError {
				requested_handle,
				offending_descriptor,
			})
		} else {
			let lock = offending_descriptor.lock();
			let session_lock_state = lock.session_borrow_state(session);

			debug_assert_ne!(session_lock_state, Some(requested_mutability));

			Self::Locked(LockPermissionsError {
				lock,
				requested_mode: requested_mutability,
				session_had_lock: session_lock_state.is_some(),
			})
		}
	}
}

#[derive(Debug, Copy, Clone, Error)]
pub struct SlotDeadError {
	pub requested_handle: LockAndMeta,
	pub offending_descriptor: LockAndMeta,
}

impl fmt::Display for SlotDeadError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"`Slot` with handle {:?} is dead",
			self.requested_handle.meta()
		)?;
		if self.requested_handle.meta() != 0 {
			write!(
				f,
				", and has been replaced by a slot with generation {:?}.",
				self.offending_descriptor.meta()
			)?;
		} else {
			f.write_str(".")?;
		}
		Ok(())
	}
}
