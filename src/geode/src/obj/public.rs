use super::gen::{ExtendedGen, IdAlloc, SessionLocks};
use super::heap::{GcHeap, Slot, SlotManager};
use crate::atomic_ref_cell::{ARef, ARefCell};
use crate::util::cell::{OnlyMut, SyncUnsafeCellMut};
use crate::util::marker::PhantomNoSync;
use crate::util::number::{AtomicNZU64Generator, NumberGenRef};
use antidote::Mutex;
use derive_where::derive_where;
use once_cell::sync::OnceCell;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ptr::Pointee;
use std::sync::Arc;

// === Globals === //

struct ObjectDB {
	/// A generator for `Obj` generations.
	// TODO: Move to `ThreadDb`.
	generation_gen: AtomicNZU64Generator,

	/// A lock indicating whether we're collecting garbage. Read references indicate that we have
	/// ongoing [Sessions](Session) while a write reference indicates that we're collecting garbage.
	/// [GcData], which holds our heaps for long-lived objects, is only available during garbage
	/// collection.
	gc_data: ARefCell<OnlyMut<GcData>>,

	/// Global state to help manage sessions. This should only ever be locked when we hold a reference
	/// lock to `gc_data`.
	sess_data: Mutex<SessData>,
}

fn object_db() -> &'static ObjectDB {
	static SINGLETON: OnceCell<ObjectDB> = OnceCell::new();

	SINGLETON.get_or_init(|| ObjectDB {
		generation_gen: AtomicNZU64Generator::default(),
		gc_data: ARefCell::new(OnlyMut::new(GcData {})),
		sess_data: Mutex::new(SessData {
			free_sessions: IdAlloc::default(),
			free_locks: IdAlloc::default(),
		}),
	})
}

struct GcData {
	// (empty for now)
}

struct SessData {
	free_sessions: IdAlloc,
	free_locks: IdAlloc,
}

/// A pointer to a database thread. Can only be promoted by either the GC routine or by the thread
/// owning it.
type DbThreadPtr = Arc<SyncUnsafeCellMut<DbThread>>;

#[derive(Default)]
struct DbThread {
	slot_manager: SlotManager,
	heap: GcHeap,
}

thread_local! {
	static THREAD_DB: DbThreadPtr = Default::default();
}

// === Sessions === //

pub struct Session<'a> {
	_lt: PhantomData<&'a ()>,
	_no_sync: PhantomNoSync,
	gc_guard: ARef<'static, OnlyMut<GcData>>,
	id: u8,
	thread: DbThreadPtr,
	lock_ids: SessionLocks,
}

impl<'a> Session<'a> {
	pub fn acquire<I>(locks: I) -> Self
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
			_no_sync: PhantomNoSync::default(),
			gc_guard,
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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(u8);

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
	pub fn new() -> (Self, Lock) {
		let token = Self::default();
		let handle = token.handle();
		(token, handle)
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

// === Obj === //

pub unsafe trait ObjPointee: 'static + Send {}

unsafe impl<T: 'static + Send> ObjPointee for T {}

#[derive_where(Copy, Clone)]
pub struct Obj<T: ?Sized + ObjPointee> {
	slot: &'static Slot,
	gen: ExtendedGen,
	meta: <T as Pointee>::Metadata,
}

impl<T: Sized + ObjPointee + Sync> Obj<T> {
	pub fn new(session: &Session, value: T) -> Self {
		Self::new_in_raw(session, 0xFF, value)
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	fn new_in_raw(session: &Session, lock: u8, value: T) -> Self {
		let thread_data = unsafe { session.thread.get_mut_unchecked() };

		// Reserve a slot for us
		let slot = thread_data.slot_manager.reserve();

		// Generate a `gen` ID
		let gen = object_db().generation_gen.generate_ref();
		let gen = ExtendedGen::new(lock, Some(gen));

		// Allocate the object
		let p_data = thread_data.heap.alloc(slot, gen, value);
		let (_base_ptr, meta) = p_data.to_raw_parts();

		// And construct the obj
		Self { slot, gen, meta }
	}
}

impl<T: ?Sized + ObjPointee> Eq for Obj<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for Obj<T> {
	fn eq(&self, other: &Self) -> bool {
		self.slot as *const Slot == other.slot as *const Slot && self.gen == other.gen
	}
}

impl<T: ?Sized + ObjPointee> Hash for Obj<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		(self.slot as *const Slot).hash(state);
		self.gen.hash(state);
	}
}
