use std::{
	alloc::Layout,
	borrow::Cow,
	cell::{Ref, RefCell, RefMut},
	fmt::{self, Write},
	hash,
	marker::Unsize,
	num::NonZeroU64,
	ptr::{self, NonNull, Pointee},
	sync::atomic::AtomicU64,
};

use arr_macro::arr;
use derive_where::derive_where;
use parking_lot::Mutex;
use thiserror::Error;

use crate::util::{
	cell::MutexedUnsafeCell,
	error::ResultExt,
	number::{LocalBatchAllocator, U8BitSet},
	ptr::unsize_meta,
	threading::new_lot_mutex,
};

use super::{
	debug::DebugLabel,
	internals::{
		gen::{ExtendedGen, SessionLocks, MAX_OBJ_GEN_EXCLUSIVE},
		heap::{GcHeap, Slot, SlotManager},
	},
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session, StaticStorage, StaticStorageHandler},
};

// === Singleton data === //

const ID_GEN_BATCH_SIZE: u64 = 4096 * 4096;

struct GlobalData {
	id_batch_gen: AtomicU64,
	sess_data: Mutex<GlobalSessData>,
}

struct GlobalSessData {
	/// A bit set of reserved locks.
	reserved_locks: U8BitSet,

	/// A bit set of locks held by a session. A lock can be held without being reserved.
	held_locks: U8BitSet,

	/// Debug names for the various locks.
	lock_names: [Option<Cow<'static, str>>; 256],
}

static GLOBAL_DATA: GlobalData = GlobalData {
	id_batch_gen: AtomicU64::new(1),
	sess_data: new_lot_mutex(GlobalSessData {
		reserved_locks: U8BitSet::new(),
		held_locks: U8BitSet::new(),
		lock_names: arr![None; 256],
	}),
};

/// Per-session data to manage [Obj] allocation.
///
/// ## Safety
///
/// For best performance, we use a [MutexedUnsafeCell] instead of a [RefCell]. However, this means
/// that we have to be very careful about avoiding reentracy. All public methods can assume that their
/// corresponding [LocalSessData] is unborrowed by the time they are called. To enforce this invariant,
/// users borrowing state from here must ensure that they never call untrusted (i.e. user) code while
/// the borrow is ongoing.
///
/// TODO: Maybe we should have debug-only tracking in `MutexedUnsafeCell` to make it easier to catch
///  bugs.
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
		let mut global_session_data = GLOBAL_DATA.sess_data.lock();
		let local_session_data = target.as_mut().unwrap().get_mut();

		for lock in local_session_data.lock_set.iter_set() {
			local_session_data.lock_set.unset(lock);
			local_session_data.session_locks.unlock(lock);
			global_session_data.held_locks.unset(lock);
		}
	}
}

// === Locks === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(u8);

struct LockFormatter<'a> {
	lock: Lock,
	sess_data: &'a GlobalSessData,
}

impl fmt::Debug for LockFormatter<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lock")
			.field("id", &self.lock.0)
			.field(
				"debug_name",
				&self.sess_data.lock_names[self.lock.0 as usize],
			)
			.finish()
	}
}

impl fmt::Debug for Lock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		LockFormatter {
			lock: *self,
			sess_data: &*GLOBAL_DATA.sess_data.lock(),
		}
		.fmt(f)
	}
}

impl Lock {
	pub fn new<L: DebugLabel>(label: L) -> Owned<Self> {
		let mut global = GLOBAL_DATA.sess_data.lock();

		// Allocate ID
		let id = global.reserved_locks.reserve_zero_bit().unwrap_or(0xFF);
		assert_ne!(
			id, 0xFF,
			"Cannot allocate more than 255 locks continuously."
		);

		// Set debug name
		global.lock_names[id as usize] = label.to_debug_label();

		// Produce wrappers
		Owned::new(Self(id))
	}

	pub fn is_held(self) -> bool {
		GLOBAL_DATA.sess_data.lock().held_locks.contains(self.0)
	}

	pub fn slot(self) -> u8 {
		self.0
	}
}

impl Destructible for Lock {
	fn destruct(self) {
		let mut global = GLOBAL_DATA.sess_data.lock();

		global.lock_names[self.slot() as usize] = None;
		global.reserved_locks.unset(self.slot());
	}
}

// === Session extensions === //

impl Session<'_> {
	pub fn acquire_locks<I: IntoIterator<Item = Lock>>(self, locks: I) {
		// We collect our locks before we enter the critical section because we really don't want
		// users running any code in the critical section. For one, it's necessary for us to prove
		// the validity of `get_mut_unchecked`. We also want to avoid deadlocks.
		let locks = locks.into_iter().collect::<Vec<_>>();

		// Acquire dependencies once
		let mut global_sess_data = GLOBAL_DATA.sess_data.lock();
		let local_sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(self).get_mut_unchecked()
		};

		for lock in locks {
			let slot = lock.slot();

			// Ignore locks that we already have.
			if local_sess_data.lock_set.contains(slot) {
				continue;
			}

			// Ensure that no one else has the lock.
			assert!(
				!global_sess_data.held_locks.contains(slot),
				"Cannot lock {:?} in more than one session.",
				// Needed to avoid implicit dead-locks.
				LockFormatter {
					lock,
					sess_data: &*global_sess_data,
				},
			);

			// Register it globally.
			global_sess_data.held_locks.set(slot);

			// Register it in both the bit set and the `session_locks` container.
			local_sess_data.lock_set.set(slot);
			local_sess_data.session_locks.lock(slot);
		}
	}

	pub fn reserve_slot_capacity(self, amount: usize) {
		let local_sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(self).get_mut_unchecked()
		};

		local_sess_data.slots.reserve_capacity(amount);
	}
}

// === Obj Errors === //

#[derive(Debug, Copy, Clone, Error)]
pub struct ObjDeadError {
	pub requested: RawObj,
	pub new_gen: u64,
}

impl fmt::Display for ObjDeadError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "`Obj` with handle {:?} is dead", self.requested)?;
		if self.new_gen != 0 {
			write!(
				f,
				", and has been replaced by an entity with generation {:?}.",
				self.new_gen
			)?;
		} else {
			f.write_char('.')?;
		}
		Ok(())
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("Obj with handle {requested:?} is locked under {lock:?}—a lock the fetch `Session` hasn't acquired")]
pub struct ObjLockedError {
	pub requested: RawObj,
	pub lock: Lock,
}

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to fetch `Obj`")]
pub enum ObjGetError {
	Dead(#[from] ObjDeadError),
	Locked(#[from] ObjLockedError),
}

impl ObjGetError {
	pub fn weak(self) -> Result<ObjDeadError, ObjLockedError> {
		match self {
			Self::Dead(value) => Ok(value),
			Self::Locked(locked) => Err(locked),
		}
	}

	pub fn unwrap_weak<T>(result: Result<T, Self>) -> Result<T, ObjDeadError> {
		result.map_err(|err| err.weak().unwrap_pretty())
	}
}

// === RawObj === //

#[derive(Copy, Clone)]
pub struct RawObj {
	slot: &'static Slot,
	gen: ExtendedGen,
}

impl fmt::Debug for RawObj {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RawObj")
			.field("slot", &(self.slot as *const Slot))
			.field("gen", &self.gen)
			.finish_non_exhaustive()
	}
}

impl hash::Hash for RawObj {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.gen.hash(state)
	}
}

impl Eq for RawObj {}

impl PartialEq for RawObj {
	fn eq(&self, other: &Self) -> bool {
		self.gen == other.gen
	}
}

impl RawObj {
	// Constructors
	pub fn new_dynamic_in(
		session: Session,
		lock: Option<Lock>,
		layout: Layout,
	) -> (Owned<Self>, NonNull<u8>) {
		let sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(session).get_mut_unchecked()
		};

		// Reserve a slot for us
		let slot = sess_data.slots.reserve();

		// Generate a `gen` ID
		let gen = unsafe {
			// Safety: TODO
			NonZeroU64::new_unchecked(sess_data.generation_gen.generate(
				&GLOBAL_DATA.id_batch_gen,
				MAX_OBJ_GEN_EXCLUSIVE,
				ID_GEN_BATCH_SIZE,
			))
		};

		// Allocate the object
		let p_data = {
			// We need to create a separate gen for the slot allocation as we do for the `Obj`.
			let gen_and_lock = ExtendedGen::new(lock.map_or(0, |l| l.0), Some(gen));

			let p_data = sess_data.heap.alloc_dynamic(slot, gen_and_lock, layout);
			p_data
		};

		// Create the proper `gen` ID
		let gen = ExtendedGen::new(0xFF, Some(gen));

		// And construct the obj
		(Owned::new(Self { slot, gen }), p_data)
	}

	pub fn new_dynamic(session: Session, layout: Layout) -> (Owned<Self>, NonNull<u8>) {
		Self::new_dynamic_in(session, None, layout)
	}

	// Fetching
	pub fn try_get_ptr(&self, session: Session) -> Result<*const (), ObjGetError> {
		let sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(session).get_mut_unchecked()
		};

		#[cold]
		#[inline(never)]
		fn decode_error(
			sess_data: &LocalSessData,
			requested: RawObj,
			slot_gen: ExtendedGen,
		) -> ObjGetError {
			let lock_id = slot_gen.meta();
			if !sess_data.session_locks.check_lock(lock_id) {
				return ObjGetError::Locked(ObjLockedError {
					requested,
					lock: Lock(lock_id),
				});
			}

			debug_assert_ne!(slot_gen.gen(), requested.gen.gen());
			ObjGetError::Dead(ObjDeadError {
				requested: requested,
				new_gen: slot_gen.gen(),
			})
		}

		match self.slot.try_get_base(&sess_data.session_locks, self.gen) {
			Ok(ptr) => Ok(ptr),
			Err(slot_gen) => Err(decode_error(&sess_data, *self, slot_gen)),
		}
	}

	pub fn get_ptr(&self, session: Session) -> *const () {
		self.try_get_ptr(session).unwrap_pretty()
	}

	pub fn weak_get_ptr(&self, session: Session) -> Result<*const (), ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get_ptr(session))
	}

	pub fn is_alive_now(&self, _session: Session) -> bool {
		self.slot.is_alive(self.gen)
	}

	pub fn destroy(&self, session: Session) {
		let sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(session).get_mut_unchecked()
		};

		if self.slot.release() {
			sess_data.slots.unreserve(self.slot);
		}
	}
}

impl Destructible for RawObj {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		});
	}
}

// === Obj === //

pub unsafe trait ObjPointee: 'static + Send {}

unsafe impl<T: ?Sized + 'static + Send> ObjPointee for T {}

#[derive_where(Copy, Clone)]
pub struct Obj<T: ?Sized + ObjPointee> {
	raw: RawObj,
	meta: <T as Pointee>::Metadata,
}

impl<T: Sized + ObjPointee + Sync> Obj<T> {
	pub fn new(session: Session, value: T) -> Owned<Self> {
		Self::new_in_raw(session, 0xFF, value)
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	pub fn new_in(session: Session, lock: Lock, value: T) -> Owned<Self> {
		Self::new_in_raw(session, lock.0, value)
	}

	fn new_in_raw(session: Session, lock: u8, value: T) -> Owned<Self> {
		// TODO: De-duplicate constructor

		let sess_data = unsafe {
			// Safety: TODO
			LocalSessData::get(session).get_mut_unchecked()
		};

		// Reserve a slot for us
		let slot = sess_data.slots.reserve();

		// Generate a `gen` ID
		let gen = unsafe {
			// Safety: TODO
			NonZeroU64::new_unchecked(sess_data.generation_gen.generate(
				&GLOBAL_DATA.id_batch_gen,
				MAX_OBJ_GEN_EXCLUSIVE,
				ID_GEN_BATCH_SIZE,
			))
		};

		// Allocate the object
		let meta = {
			// We need to create a separate gen for the slot allocation as we do for the `Obj`.
			let gen_and_lock = ExtendedGen::new(lock, Some(gen));

			let p_data = sess_data.heap.alloc_static(slot, gen_and_lock, value);
			let (_, meta) = p_data.to_raw_parts();
			meta
		};

		// Create the proper `gen` ID
		let gen = ExtendedGen::new(0xFF, Some(gen));

		// And construct the obj
		Owned::new(Self {
			raw: RawObj { slot, gen },
			meta,
		})
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	pub fn try_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjGetError> {
		let base = self.raw.try_get_ptr(session)?;
		let ptr = ptr::from_raw_parts::<T>(base, self.meta);

		Ok(unsafe { &*ptr })
	}

	pub fn get<'a>(&self, session: Session<'a>) -> &'a T {
		self.try_get(session).unwrap_pretty()
	}

	pub fn weak_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get(session))
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.raw.is_alive_now(session)
	}

	pub fn destroy<'a>(&self, session: Session<'a>) {
		self.raw.destroy(session)
	}

	pub fn as_raw(&self) -> RawObj {
		self.raw
	}

	pub fn as_unsized<U>(&self) -> Obj<U>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		Obj {
			raw: self.raw,
			meta: unsize_meta::<T, U>(self.meta),
		}
	}
}

impl<T: ?Sized + ObjPointee> fmt::Debug for Obj<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Obj")
			.field("raw", &self.raw)
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized + ObjPointee> Eq for Obj<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for Obj<T> {
	fn eq(&self, other: &Self) -> bool {
		self.raw == other.raw
	}
}

impl<T: ?Sized + ObjPointee> hash::Hash for Obj<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.raw.hash(state);
	}
}

impl<T: ?Sized + ObjPointee> Destructible for Obj<T> {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		})
	}
}

// === Obj extensions === //

pub type ObjRw<T> = Obj<RefCell<T>>;

impl<T: ObjPointee> ObjRw<T> {
	pub fn new_rw(session: Session, lock: Lock, value: T) -> Owned<Self> {
		Self::new_in(session, lock, RefCell::new(value))
	}
}

impl<T: ?Sized + ObjPointee> ObjRw<T> {
	pub fn borrow<'a>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.get(session).borrow()
	}

	pub fn borrow_mut<'a>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.get(session).borrow_mut()
	}
}

pub trait ObjCtorExt: Sized + ObjPointee {
	fn box_obj(self, session: Session) -> Owned<Obj<Self>>
	where
		Self: Sync,
	{
		Obj::new(session, self)
	}

	fn box_obj_in(self, session: Session, lock: Lock) -> Owned<Obj<Self>> {
		Obj::new_in(session, lock, self)
	}

	fn box_obj_rw(self, session: Session, lock: Lock) -> Owned<Obj<RefCell<Self>>> {
		Obj::new_rw(session, lock, self)
	}
}

impl<T: Sized + ObjPointee> ObjCtorExt for T {}

impl<T: ?Sized + ObjPointee> Owned<Obj<T>> {
	pub fn to_unsized<U>(self) -> Owned<Obj<U>>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		Owned::new(self.manually_manage().as_unsized::<U>())
	}

	pub fn to_raw(self) -> Owned<RawObj> {
		Owned::new(self.manually_manage().as_raw())
	}
}