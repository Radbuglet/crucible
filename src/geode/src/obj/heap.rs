use super::gen::{ExtendedGen, SessionLocks};
use super::{Lock, ObjGetError, ObjLockedError, RawObj};
use crate::obj::ObjDeadError;
use crate::util::bump::LeakyBump;
use bumpalo::Bump;
use std::alloc::Layout;
use std::ptr::{null, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

// === SlotManager === //

#[derive(Default)]
pub struct SlotManager {
	/// A `Bump` allocator from which we allocate slots on this thread. The memory owned by the bump
	/// will never be released so make sure to reuse `SlotManagers`!
	bump: LeakyBump,

	/// A list of slots from which we can allocate. Slots re-enter this pool immediately upon being
	/// freed to minimize the amount of new slots we have to create.
	free: Vec<&'static Slot>,
}

impl SlotManager {
	pub fn reserve(&mut self) -> &'static Slot {
		if let Some(free) = self.free.pop() {
			free
		} else {
			self.bump.alloc(Slot::default())
		}
	}

	pub fn unreserve(&mut self, slot: &'static Slot) {
		self.free.push(slot);
	}
}

#[derive(Default)]
pub struct Slot {
	lock_and_gen: AtomicU64,
	base_ptr: AtomicPtr<()>,
}

impl Slot {
	pub fn acquire(&self, new_gen: ExtendedGen, new_base: *const ()) {
		debug_assert_ne!(new_gen.gen(), 0);
		self.acquire_raw(new_gen.raw(), new_base);
	}

	pub fn release(&self) {
		self.acquire_raw(ExtendedGen::new(0, None).raw(), null());
	}

	fn acquire_raw(&self, new_gen: u64, new_base: *const ()) {
		// We first ensure that the new `lock_and_gen` is visible to other threads before modifying
		// the pointer. That way, even if they load the stale pointer, they'll see that the `gen`
		// has been changed and prevent the unsafe fetch.
		self.lock_and_gen.store(new_gen, Ordering::Relaxed);
		self.base_ptr.store(new_base as *mut (), Ordering::Release); // Forces other cores to see `lock_and_gen`
	}

	pub fn try_get_base(raw: RawObj, locks: &SessionLocks) -> Result<*const (), ObjGetError> {
		let RawObj { slot, gen: ptr_gen } = raw;

		let base_ptr = slot.base_ptr.load(Ordering::Acquire); // Forces other cores to see `lock_and_gen` and `base_ptr`.
		let slot_gen = slot.lock_and_gen.load(Ordering::Relaxed);
		let slot_gen = ExtendedGen::from_raw(slot_gen);

		if locks.check_gen_and_lock(ptr_gen, slot_gen) {
			Ok(base_ptr)
		} else {
			// It is possible for both the lock and liveness checks to fail. However, because lock
			// errors are typically indicative of a bug while liveness are regular events, we want
			// to give locks priority.
			if !locks.check_lock(slot_gen.meta()) {
				return Err(ObjGetError::Locked(ObjLockedError {
					requested: raw,
					lock: Lock(slot_gen.meta()),
				}));
			}

			// Because locks weren't at fault, the generation should be.
			debug_assert_ne!(slot_gen.gen(), ptr_gen.gen());

			Err(ObjGetError::Dead(ObjDeadError {
				requested: raw,
				new_gen: slot_gen.gen(),
			}))
		}
	}

	pub fn is_alive(&self, ptr_gen: ExtendedGen) -> bool {
		let curr_gen = self.lock_and_gen.load(Ordering::Relaxed);
		let curr_gen = ExtendedGen::from_raw(curr_gen);
		ptr_gen.gen() == curr_gen.gen()
	}
}

// === GcHeap === //

#[derive(Default)]
pub struct GcHeap {
	bump: Bump,
}

impl GcHeap {
	pub fn alloc_static<T>(
		&mut self,
		slot: &'static Slot,
		gen_and_lock: ExtendedGen,
		value: T,
	) -> *const T {
		let full_ptr = self.bump.alloc(value) as *const T;
		let base_ptr = full_ptr as *const ();

		slot.acquire(gen_and_lock, base_ptr);
		full_ptr
	}

	pub fn alloc_dynamic(
		&mut self,
		slot: &'static Slot,
		gen_and_lock: ExtendedGen,
		layout: Layout,
	) -> NonNull<u8> {
		let full_ptr = self.bump.alloc_layout(layout);
		slot.acquire(gen_and_lock, full_ptr.as_ptr() as *const ());
		full_ptr
	}
}
