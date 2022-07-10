//! The low-level implementation underlying `Obj`, written in such a way that control flow is as
//! explicit as h&umanly possible. This module exists purely to separate reentrancy-sensitive code
//! from userland objects. The latter has much more complicated implicit control flow (e.g. `Drop`
//! handlers, accidental `Debug` calls, etc) that could easily cause deadlocks or UB if we're not
//! careful.

use std::{alloc::Layout, borrow::Cow, fmt, sync::atomic::AtomicU64};

use crucible_core::{array::arr, cell::MutexedUnsafeCell};
use parking_lot::Mutex;

use crate::{
	core::reflect::ReflectType,
	core::session::{Session, StaticStorage, StaticStorageHandler},
	util::{
		number::{LocalBatchAllocator, U8BitSet},
		threading::new_lot_mutex,
	},
};

use super::{
	gen::{ExtendedGen, SessionLocks, MAX_OBJ_GEN_EXCLUSIVE},
	heap::{GcHeap, Slot, SlotManager},
};

// === Singletons === //

const ID_GEN_BATCH_SIZE: u64 = 4096 * 4096;

struct GlobalData {
	id_batch_gen: AtomicU64,
	mutexed: Mutex<GlobalMutexedData>,
}

struct GlobalMutexedData {
	/// A bit set of reserved locks.
	reserved_locks: U8BitSet,

	/// A bit set of locks held by a session. A lock can be held without being reserved.
	held_locks: U8BitSet,

	/// Debug names for the various locks.
	lock_names: [Option<Cow<'static, str>>; 256],
}

static GLOBAL_DATA: GlobalData = GlobalData {
	id_batch_gen: AtomicU64::new(1),
	mutexed: new_lot_mutex(GlobalMutexedData {
		reserved_locks: U8BitSet::new(),
		held_locks: U8BitSet::new(),
		lock_names: arr![None; 256],
	}),
};

/// Per-session data to manage `Obj` allocation.
///
/// ## Safety
///
/// For best performance, we use a [MutexedUnsafeCell] instead of a [RefCell](std::cell::RefCell).
/// However, this means that we have to be very careful about avoiding reentracy. All public methods
/// can assume that their corresponding [LocalSessData] is unborrowed by the time they are called.
/// To enforce this invariant, users borrowing state from here must ensure that they never call
/// untrusted (i.e. user) code while the borrow is ongoing.
///
#[derive(Default)]
pub(crate) struct LocalSessData {
	/// Our local garbage-collected heap that serves as both a nursery for new allocations and the
	/// primary heap for objects that don't belong in a specific global heap.
	heap: GcHeap,

	/// A free stack of [Slot]s to be reused.
	slots: SlotManager,

	/// Our session's actively held lock set.
	session_locks: SessionLocks,

	/// Our session's thread-local generation allocator.
	generation_gen: LocalBatchAllocator,

	/// The set of all locks acquired in our [SessionLocks] set.
	lock_set: U8BitSet,
}

impl StaticStorageHandler for LocalSessData {
	type Comp = MutexedUnsafeCell<Self>;

	fn init_comp(target: &mut Option<Self::Comp>) {
		if target.is_none() {
			*target = Some(Default::default());
		}
	}

	fn deinit_comp(target: &mut Option<Self::Comp>) {
		let mut global_session_data = GLOBAL_DATA.mutexed.lock();
		let local_session_data = target.as_mut().unwrap().get_mut();

		for lock in local_session_data.lock_set.iter_set() {
			local_session_data.lock_set.unset(lock);
			local_session_data.session_locks.unlock(lock);
			global_session_data.held_locks.unset(lock);
		}
	}
}

// === Lock management === //

pub fn reserve_lock(label: Option<Cow<'static, str>>) -> u8 {
	let mut global_data = GLOBAL_DATA.mutexed.lock();

	let id = global_data
		.reserved_locks
		.reserve_zero_bit()
		.unwrap_or(0xFF);

	assert_ne!(
		id, 0xFF,
		"Cannot allocate more than 255 locks concurrently."
	);

	global_data.lock_names[id as usize] = label;

	id
}

pub fn unreserve_lock(handle: u8) {
	GLOBAL_DATA.mutexed.lock().reserved_locks.unset(handle);
}

pub fn is_lock_held_somewhere(handle: u8) -> bool {
	GLOBAL_DATA.mutexed.lock().held_locks.contains(handle)
}

pub fn is_lock_held_by(session: Session<'_>, handle: u8) -> bool {
	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};

	local_sess_data.lock_set.contains(handle)
}

pub fn acquire_locks(session: Session<'_>, locks: &[u8]) {
	let mut mutexed_data = GLOBAL_DATA.mutexed.lock();

	// Acquire local session data (enter non-reentrant section!)
	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};

	// First, ensure that none of the locks are held.
	for lock in locks.iter().copied() {
		assert!(
			!mutexed_data.held_locks.contains(lock),
			"Failed to acquire lock with ID {} (debug name \"{}\"): already held by another session.",
			lock,
			LockDebugNameWithCx {
				id: lock,
				mutexed_data: &*mutexed_data,
			}
		);
	}

	// Now, lock them!
	for lock in locks.iter().copied() {
		mutexed_data.held_locks.set(lock);
		local_sess_data.session_locks.lock(lock);
		local_sess_data.lock_set.set(lock);
	}
}

pub fn get_lock_debug_name(id: u8) -> LockDebugName {
	LockDebugName(id)
}

#[derive(Clone)]
pub struct LockDebugName(u8);

impl fmt::Debug for LockDebugName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		LockDebugNameWithCx {
			id: self.0,
			mutexed_data: &*GLOBAL_DATA.mutexed.lock(),
		}
		.fmt(f)
	}
}

impl fmt::Display for LockDebugName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self, f)
	}
}

#[derive(Clone)]
struct LockDebugNameWithCx<'a> {
	id: u8,
	mutexed_data: &'a GlobalMutexedData,
}

impl fmt::Debug for LockDebugNameWithCx<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.mutexed_data.lock_names[self.id as usize] {
			Some(debug_name) => Some::<&str>(&*debug_name).fmt(f),
			None => None::<&str>.fmt(f),
		}
	}
}

impl fmt::Display for LockDebugNameWithCx<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self, f)
	}
}

// === Obj management === //

pub fn reserve_obj_slot_capacity(session: Session<'_>, amount: usize) {
	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};
	local_sess_data.slots.reserve_capacity(amount);
}

#[inline(always)]
pub fn allocate_new_obj(
	session: Session<'_>,
	ty: &'static ReflectType,
	layout: Layout,
	lock_id: u8,
) -> (&'static Slot, ExtendedGen, *mut ()) {
	// Ensure that `ty.layout` and `layout` are consistent in debug builds.
	debug_assert!(
		ty.static_layout
			.map_or(true, |static_layout| static_layout == layout),
		"`ty`-provided `Layout` does not match provided `layout`."
	);

	// Acquire local session data (enter non-reentrant section!)
	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};

	// Generate a new ID
	let gen = local_sess_data.generation_gen.generate(
		&GLOBAL_DATA.id_batch_gen,
		MAX_OBJ_GEN_EXCLUSIVE,
		ID_GEN_BATCH_SIZE,
	);
	debug_assert_ne!(gen, 0);

	// Acquire a slot
	let slot = local_sess_data.slots.reserve();

	// Lock that slot and reserve space for the allocation.
	let gen_and_lock = ExtendedGen::new(lock_id, gen);
	let full_ptr = local_sess_data
		.heap
		.alloc(slot, gen_and_lock, ty, layout)
		.cast::<()>()
		.as_ptr();

	let gen_and_mask = ExtendedGen::new(0xFF, gen);

	(slot, gen_and_mask, full_ptr)
}

#[inline(always)]
pub fn try_get_obj_ptr(
	session: Session<'_>,
	slot: &'static Slot,
	gen: ExtendedGen,
) -> Result<*const (), ExtendedGen> {
	debug_assert_eq!(gen.meta(), 0xFF);

	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};

	slot.try_get_base(&local_sess_data.session_locks, gen)
}

pub fn destroy_obj(session: Session<'_>, slot: &'static Slot, local_gen: ExtendedGen) -> bool {
	let local_sess_data = unsafe {
		// Safety: see item comment for [LocalSessData].
		LocalSessData::get(session).get_mut_unchecked()
	};

	if slot.release(local_gen) {
		local_sess_data.slots.unreserve(slot);
		true
	} else {
		false
	}
}

// === Garbage collection === //

pub fn _collect_garbage() {
	// First, we run our finalizers.

	// Next, we run our post-finalization listeners.

	// Now, we run compaction on our various heaps.

	// Finally, let's run our post-compaction listeners.
}
