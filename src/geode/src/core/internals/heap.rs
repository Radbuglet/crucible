use super::gen::{ExtendedGen, SessionLocks};
use crate::util::bump::LeakyBump;
use bumpalo::Bump;
use std::alloc::Layout;
use std::ptr::{null, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

// === SlotManager === //

/// An object responsible for allocating [Slots](Slot). The backing memory of every slot will be
/// allocated forever. It is therefore critical to ensure that you are reusing these instances.
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
	pub fn reserve_capacity(&mut self, amount: usize) {
		let extra = amount - self.free.len();

		self.free
			.extend(std::iter::repeat_with(|| self.bump.alloc(Slot::default())).take(extra));
	}

	pub fn reserve(&mut self) -> &'static Slot {
		if let Some(free) = self.free.pop() {
			free
		} else {
			self.reserve_slow()
		}
	}

	#[cold]
	fn reserve_slow(&mut self) -> &'static Slot {
		self.bump.alloc(Slot::default())
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
	/// Acquires a [Slot] by atomically assigning it a new generation and base.
	///
	/// ## Safety
	///
	/// While the object guarantees that a wrong pointer-generation pair cannot be observed, this
	/// guarantee only applies if you `acquire` the [Slot] from exactly one thread.
	pub fn acquire(&self, new_gen: ExtendedGen, new_base: *const ()) {
		debug_assert_ne!(new_gen.gen(), 0);
		self.update(new_gen, new_base);
	}

	/// Atomically releases the [Slot], invalidating both its generation and its pointer.
	///
	/// ## Safety
	///
	/// The same synchronization caveats as [Slot::acquire] apply here as well.
	pub fn release(&self) {
		self.update(ExtendedGen::new(0, None), null());
	}

	fn update(&self, new_gen: ExtendedGen, new_base: *const ()) {
		// We first ensure that the new `lock_and_gen` is visible to other threads before modifying
		// the pointer. That way, even if they load the stale pointer, they'll see that the `gen`
		// has been changed and prevent the unsafe fetch.
		self.lock_and_gen.store(new_gen.raw(), Ordering::Relaxed);
		self.base_ptr.store(new_base as *mut (), Ordering::Release); // Forces other cores to see `lock_and_gen`
	}

	// /// Attempts a best-effort repointing of the slot without invalidating the generation.
	// ///
	// /// ## Safety
	// ///
	// /// Note that this is distinct from a "reallocation" because you are not expected to invalidate
	// /// the previous version of the object. Indeed, because this is a "best-effort" repointing, other
	// /// threads might not even see the moved base pointer before the memory is deallocated. And
	// /// generally, in this model, we don't deallocate memory until garbage collection anyways.
	// pub fn repoint(&self, new_base: *const ()) {
	// 	self.base_ptr.store(new_base as *mut (), Ordering::Relaxed);
	// }

	/// Attempts to get the current base pointer for the [Slot]. Fails and returns the existing
	/// generation-lock pair if we either lack lock permission or if the slot has been generationally
	/// invalidated.
	pub fn try_get_base(
		&self,
		locks: &SessionLocks,
		ptr_gen: ExtendedGen,
	) -> Result<*const (), ExtendedGen> {
		let base_ptr = self.base_ptr.load(Ordering::Acquire); // Forces us to see `lock_and_gen` and `base_ptr`.
		let slot_gen = self.lock_and_gen.load(Ordering::Relaxed);
		let slot_gen = ExtendedGen::from_raw(slot_gen);

		if locks.check_gen_and_lock(ptr_gen, slot_gen) {
			Ok(base_ptr)
		} else {
			Err(slot_gen)
		}
	}

	/// Checks if the [Slot] is currently alive but makes zero guarantees about the future.
	///
	/// ## Safety
	///
	/// Specifically, we have no clue if the slot will remain alive when we run [Slot::try_get_base].
	/// This is purely just a heuristic.
	pub fn is_alive(&self, ptr_gen: ExtendedGen) -> bool {
		let curr_gen = self.lock_and_gen.load(Ordering::Relaxed);
		let curr_gen = ExtendedGen::from_raw(curr_gen);
		ptr_gen.gen() == curr_gen.gen()
	}
}

// === GcHeap === //

/// A garbage collected heap.
#[derive(Default)]
pub struct GcHeap {
	bump: Bump,
}

impl GcHeap {
	pub fn alloc_static<T: 'static>(
		&mut self,
		slot: &'static Slot,
		gen_and_lock: ExtendedGen,
		value: T,
	) -> *const T {
		//self.meta_bump.alloc((slot, TypeMeta::of::<T>()));
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
