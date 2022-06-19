use super::gen::{ExtendedGen, SessionLocks, MAX_OBJ_GEN_EXCLUSIVE};
use super::heap::{GcHeap, Slot, SlotManager};
use crate::atomic_ref_cell::{ARef, ARefCell};
use crate::util::cell::{OnlyMut, SyncUnsafeCellMut};
use crate::util::error::ResultExt;
use crate::util::marker::PhantomNoSendOrSync;
use crate::util::number::{LocalBatchAllocator, U8Alloc};
use antidote::Mutex;
use arr_macro::arr;
use derive_where::derive_where;
use once_cell::sync::OnceCell;
use std::alloc::Layout;
use std::cell::{Ref, RefCell, RefMut};
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ptr::{NonNull, Pointee};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use thiserror::Error;

// === Globals === //

const ID_GEN_BATCH_SIZE: u64 = 4096;

/// Application-global static data. Because cross-thread synchronization is costly, almost everything
/// is either thread-local or [Session]-local ([Slot] is a significant exception, but it uses
/// relatively lightweight `Relaxed` and `Acquire` fetches for its fast-path operations). This
/// singleton is mainly used to register and reuse [DbThread] instances, generate and manage IDs for
/// long-lived objects, and by the garbage collector to store heaps.
///
/// The lock order is as follows:
///
/// - `gc_data`
/// - `sess_data`
///
/// It is perfectly acceptible to just lock `sess_data` without locking `gc_data` (just remember that
/// `gc_data` implicitly locks [DbThread] state as well!), just so long as you don't continue to lock
/// `gc_data` afterwards.
struct ObjectDB {
	/// A generator for `Obj` generation batches.
	generation_batch_gen: AtomicU64,

	/// A lock indicating whether we're collecting garbage. Read references indicate that we have
	/// ongoing [Sessions](Session) while a write reference indicates that we're collecting garbage.
	/// [GcData], which holds our heaps for long-lived objects, is only available during garbage
	/// collection.
	gc_data: ARefCell<OnlyMut<GcData>>,

	/// Global state to help manage sessions. See item docs for lock order.
	sess_data: Mutex<SessData>,
}

fn object_db() -> &'static ObjectDB {
	static SINGLETON: OnceCell<ObjectDB> = OnceCell::new();

	SINGLETON.get_or_init(|| ObjectDB {
		generation_batch_gen: AtomicU64::new(1),
		gc_data: ARefCell::new(OnlyMut::new(GcData {})),
		sess_data: Mutex::new(SessData {
			free_sessions: U8Alloc::default(),
			free_locks: U8Alloc::default(),
			lock_names: Box::new(arr![None; 256]),
		}),
	})
}

struct GcData {
	// (empty for now)
}

struct SessData {
	free_sessions: U8Alloc,
	free_locks: U8Alloc,
	lock_names: Box<[Option<String>; 256]>,
}

/// A pointer to a database thread. Can only be promoted by either the GC routine or by the thread
/// owning it.
type DbThreadPtr = Arc<SyncUnsafeCellMut<DbThread>>;

struct DbThread {
	object_db: &'static ObjectDB,
	slot_manager: SlotManager,
	heap: GcHeap,
	id_gen: LocalBatchAllocator,
}

thread_local! {
	static THREAD_DB: DbThreadPtr = Arc::new(SyncUnsafeCellMut::new(DbThread {
		object_db: object_db(),
		slot_manager: SlotManager::default(),
		heap: GcHeap::default(),
		id_gen: LocalBatchAllocator::default(),
	}));
}

// === Sessions === //

pub struct Session<'a> {
	_lt: PhantomData<&'a ()>,
	// Can't be `Sync` because we real on the thread unsafety of `Session` to allow us to assert
	// that a thread's session owns a lock. Can't be `Send` either because `Sessions` acquire thread
	// local data and use it in an unsynchronized manner.
	_no_threading: PhantomNoSendOrSync,
	_gc_guard: ARef<'static, OnlyMut<GcData>>,
	id: u8,
	thread: DbThreadPtr,
	lock_ids: SessionLocks,
}

impl<'a> Session<'a> {
	pub fn new<I>(locks: I) -> Self
	where
		I: IntoIterator<Item = &'a mut LockToken>,
	{
		// Ensure that the GC is not running concurrently.
		let gc_guard = object_db()
			.gc_data
			.try_borrow()
			.expect("Cannot create a session while garbage collection is in progress.");

		// Acquire the session data.
		let mut sess_data = object_db().sess_data.lock();

		// Register a session ID.
		let id = sess_data.free_sessions.alloc();
		assert!(
			id != 255,
			"Cannot create more than 256 concurrent sessions!"
		);

		// Cache our TLS `THREAD_DB`.
		let thread = THREAD_DB.with(|p| p.clone());

		// Populate `SessionLocks`
		let mut lock_ids = SessionLocks::default();
		for lock in locks {
			lock_ids.lock(lock.handle().0);
		}

		// Construct the service
		Self {
			_lt: PhantomData,
			_no_threading: PhantomNoSendOrSync::default(),
			_gc_guard: gc_guard,
			id,
			thread,
			lock_ids,
		}
	}
}

impl Drop for Session<'_> {
	fn drop(&mut self) {
		let mut sess_data = object_db().sess_data.lock();
		sess_data.free_locks.free(self.id);
	}
}

// === Lock === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(u8);

impl Lock {
	pub fn debug_name(&self) -> Option<String> {
		object_db().sess_data.lock().lock_names[self.0 as usize].clone()
	}
}

impl Debug for Lock {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let sess_data = object_db().sess_data.lock();

		f.debug_struct("Lock")
			.field("id", &self.0)
			.field("debug_name", &sess_data.lock_names[self.0 as usize])
			.finish()
	}
}

#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct LockToken(Lock);

impl Default for LockToken {
	fn default() -> Self {
		let mut sess_data = object_db().sess_data.lock();
		let id = sess_data.free_sessions.alloc();
		assert!(id != 255, "Cannot create more than 256 concurrent locks!");

		Self(Lock(id))
	}
}

impl LockToken {
	pub fn new(name: Option<String>) -> (Self, Lock) {
		let token = Self::default();
		let handle = token.handle();

		object_db().sess_data.lock().lock_names[handle.0 as usize] = name;

		(token, handle)
	}

	pub fn debug_name(&self) -> Option<String> {
		self.handle().debug_name()
	}

	pub fn set_debug_name(&self, name: Option<String>) {
		object_db().sess_data.lock().lock_names[self.handle().0 as usize] = name;
	}

	pub fn handle(&self) -> Lock {
		self.0
	}
}

impl Drop for LockToken {
	fn drop(&mut self) {
		let mut sess_data = object_db().sess_data.lock();
		sess_data.free_sessions.free(self.handle().0);
	}
}

// === Error types === //

#[derive(Debug, Copy, Clone, Error)]
#[error("Obj with handle {requested:?} is dead, and has been replaced by an entity with generation {new_gen:?}")]
pub struct ObjDeadError {
	pub requested: RawObj,
	pub new_gen: u64,
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

impl Debug for RawObj {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RawObj")
			.field("slot", &(self.slot as *const Slot))
			.field("gen", &self.gen)
			.finish()
	}
}

impl Hash for RawObj {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		(self.slot as *const Slot).hash(state);
		self.gen.hash(state);
	}
}

impl Eq for RawObj {}

impl PartialEq for RawObj {
	fn eq(&self, other: &Self) -> bool {
		// Generations are never repeated.
		self.gen == other.gen
	}
}

impl RawObj {
	pub fn new_dynamic(
		session: &Session,
		lock: Option<Lock>,
		layout: Layout,
	) -> (Self, NonNull<u8>) {
		let thread_data = unsafe {
			// Safety: `new_in_raw` is non-reentrant (we never make calls to userland code, nor do
			// any library methods already acquiring this object) and `session`s holding pointers
			// to this thread's `ThreadDb` instance are non `Send`.
			session.thread.get_mut_unchecked()
		};

		// Reserve a slot for us
		let slot = thread_data.slot_manager.reserve();

		// Generate a `gen` ID
		let gen = NonZeroU64::new(thread_data.id_gen.generate(
			&thread_data.object_db.generation_batch_gen,
			MAX_OBJ_GEN_EXCLUSIVE,
			ID_GEN_BATCH_SIZE,
		))
		.unwrap();

		// Allocate the object
		let p_data = {
			// We need to create a separate gen for the slot allocation as we do for the `Obj`.
			let gen_and_lock = ExtendedGen::new(lock.map_or(255, |lock| lock.0), Some(gen));

			let p_data = thread_data.heap.alloc_dynamic(slot, gen_and_lock, layout);
			p_data
		};

		// Create the proper `gen` ID
		let gen = ExtendedGen::new(0xFF, Some(gen));

		// And construct the obj
		(Self { slot, gen }, p_data)
	}

	pub fn try_get_ptr(&self, session: &Session) -> Result<*const (), ObjGetError> {
		match self.slot.try_get_base(&session.lock_ids, self.gen) {
			Ok(ptr) => Ok(ptr),
			Err(slot_gen) => {
				let lock_id = slot_gen.meta();
				if !session.lock_ids.check_lock(lock_id) {
					return Err(ObjGetError::Locked(ObjLockedError {
						requested: *self,
						lock: Lock(lock_id),
					}));
				}

				debug_assert_ne!(slot_gen.gen(), self.gen.gen());
				Err(ObjGetError::Dead(ObjDeadError {
					requested: *self,
					new_gen: self.gen.gen(),
				}))
			}
		}
	}

	pub fn get_ptr(&self, session: &Session) -> *const () {
		self.try_get_ptr(session).unwrap_pretty()
	}

	pub fn weak_get_ptr(&self, session: &Session) -> Result<*const (), ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get_ptr(session))
	}

	pub fn is_alive_now(&self, _session: &Session) -> bool {
		self.slot.is_alive(self.gen)
	}

	pub fn destroy(&self, session: &Session) {
		let thread_data = unsafe { session.thread.get_mut_unchecked() };

		self.slot.release();
		thread_data.slot_manager.unreserve(self.slot);
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
	pub fn new(session: &Session, value: T) -> Self {
		Self::new_in_raw(session, 0xFF, value)
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	pub fn new_in(session: &Session, lock: Lock, value: T) -> Self {
		Self::new_in_raw(session, lock.0, value)
	}

	fn new_in_raw(session: &Session, lock: u8, value: T) -> Self {
		// TODO: De-duplicate constructor

		let thread_data = unsafe {
			// Safety: `new_in_raw` is non-reentrant (we never make calls to userland code, nor do
			// any library methods already acquiring this object) and `session`s holding pointers
			// to this thread's `ThreadDb` instance are non `Send`.
			session.thread.get_mut_unchecked()
		};

		// Reserve a slot for us
		let slot = thread_data.slot_manager.reserve();

		// Generate a `gen` ID
		let gen = NonZeroU64::new(thread_data.id_gen.generate(
			&thread_data.object_db.generation_batch_gen,
			MAX_OBJ_GEN_EXCLUSIVE,
			ID_GEN_BATCH_SIZE,
		))
		.unwrap();

		// Allocate the object
		let meta = {
			// We need to create a separate gen for the slot allocation as we do for the `Obj`.
			let gen_and_lock = ExtendedGen::new(lock, Some(gen));

			let p_data = thread_data.heap.alloc_static(slot, gen_and_lock, value);
			let (_, meta) = p_data.to_raw_parts();
			meta
		};

		// Create the proper `gen` ID
		let gen = ExtendedGen::new(0xFF, Some(gen));

		// And construct the obj
		Self {
			raw: RawObj { slot, gen },
			meta,
		}
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	pub fn try_get<'a>(&self, session: &'a Session) -> Result<&'a T, ObjGetError> {
		let base = self.raw.try_get_ptr(session)?;
		let ptr = std::ptr::from_raw_parts::<T>(base, self.meta);

		Ok(unsafe { &*ptr })
	}

	pub fn get<'a>(&self, session: &'a Session) -> &'a T {
		self.try_get(session).unwrap_pretty()
	}

	pub fn weak_get<'a>(&self, session: &'a Session) -> Result<&'a T, ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get(session))
	}

	pub fn is_alive_now(&self, session: &Session) -> bool {
		self.raw.is_alive_now(session)
	}

	pub fn destroy<'a>(&self, session: &'a Session) {
		self.raw.destroy(session)
	}

	pub fn raw(&self) -> RawObj {
		self.raw
	}
}

impl<T: ?Sized + ObjPointee> Debug for Obj<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl<T: ?Sized + ObjPointee> Hash for Obj<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.raw.hash(state);
	}
}

// === Obj extensions === //

pub type ObjRw<T> = Obj<RefCell<T>>;

impl<T: ObjPointee> ObjRw<T> {
	pub fn new_rw(session: &Session, lock: Lock, value: T) -> Self {
		Self::new_in(session, lock, RefCell::new(value))
	}
}

impl<T: ?Sized + ObjPointee> ObjRw<T> {
	pub fn borrow<'a>(&self, session: &'a Session) -> Ref<'a, T> {
		self.get(session).borrow()
	}

	pub fn borrow_mut<'a>(&self, session: &'a Session) -> RefMut<'a, T> {
		self.get(session).borrow_mut()
	}
}

pub trait ObjCtorExt: Sized + ObjPointee {
	fn as_obj(self, session: &Session) -> Obj<Self>
	where
		Self: Sync,
	{
		Obj::new(session, self)
	}

	fn as_obj_in(self, session: &Session, lock: Lock) -> Obj<Self> {
		Obj::new_in(session, lock, self)
	}

	fn as_obj_rw(self, session: &Session, lock: Lock) -> Obj<RefCell<Self>> {
		Obj::new_rw(session, lock, self)
	}
}

impl<T: Sized + ObjPointee> ObjCtorExt for T {}
