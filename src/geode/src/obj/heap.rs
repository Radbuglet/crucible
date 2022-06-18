use super::gen::{ExtendedGen, SessionLocks};
use super::ObjGetError;
use crate::util::{bump::LeakyBump, reflect::TypeMeta};
use bumpalo::Bump;
use std::ptr::null;
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

	pub fn try_get_base(
		&self,
		locks: &SessionLocks,
		ptr_gen: ExtendedGen,
	) -> Result<*const (), ObjGetError> {
		let base_ptr = self.base_ptr.load(Ordering::Acquire); // Forces other cores to see `lock_and_gen` and `base_ptr`.
		let slot_gen = self.lock_and_gen.load(Ordering::Relaxed);
		let slot_gen = ExtendedGen::from_raw(slot_gen);

		if locks.check(ptr_gen, slot_gen) {
			Ok(base_ptr)
		} else {
			Err(ObjGetError {})
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
	entries: Vec<GcEntry>,
}

struct GcEntry {
	slot: &'static Slot,
	base_ptr: *const (),
	ty_meta: &'static TypeMeta,
}

impl GcHeap {
	pub fn alloc<T>(&mut self, slot: &'static Slot, gen: ExtendedGen, value: T) -> *const T {
		let full_ptr = self.bump.alloc(value) as *const T;
		let base_ptr = full_ptr as *const ();
		let ty_meta = TypeMeta::of::<T>();

		// TODO: For some reason, uncommenting this line makes the allocation routine run 6 times
		// slower. We need to find a better way to query the allocations in a heap (e.g. storing the
		// meta in-band)
		// self.entries.push(GcEntry {
		// 	slot,
		// 	base_ptr,
		// 	ty_meta,
		// });
		slot.acquire(gen, base_ptr);
		full_ptr
	}

	pub fn collect_garbage(&mut self) {
		todo!()
	}
}
