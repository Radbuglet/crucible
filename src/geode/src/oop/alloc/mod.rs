use crate::util::cell::{SyncPtr, SyncUnsafeCell, SyncUnsafeCellMut};
use crate::util::marker::PhantomNoSync;
use antidote::{Mutex, RwLock, RwLockReadGuard};
use bumpalo::Bump;
use derive_where::derive_where;
use gen::{ExtendedGen, IdAlloc, SessionLocks};
use once_cell::sync::OnceCell;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ptr::{NonNull, Pointee};
use std::sync::Arc;

mod gen;

// === Global state === //

struct ObjectDB {
	gc_lock: RwLock<()>,
	gc_data: SyncUnsafeCellMut<GcData>,
	sess_data: Mutex<SessData>,
}

fn object_db() -> &'static ObjectDB {
	static SINGLETON: OnceCell<ObjectDB> = OnceCell::new();

	SINGLETON.get_or_init(|| ObjectDB {
		gc_lock: RwLock::new(()),
		gc_data: SyncUnsafeCellMut::new(GcData {}),
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

type DbThreadPtr = Arc<SyncUnsafeCellMut<DbThread>>;

#[derive(Default)]
struct DbThread {
	nursery: Bump,
	slot_alloc: Bump,
	free_slots: Vec<&'static SlotData>,
}

struct SlotData {
	base: SyncUnsafeCell<SyncPtr<NonNull<()>>>,
	gen_and_lock: SyncUnsafeCell<ExtendedGen>,
}

thread_local! {
	static THREAD_DB: DbThreadPtr = Default::default();
}

// === GC Api === //

pub mod gc {
	// TODO
}

// === Sessions === //

pub struct Session<'a> {
	_lt: PhantomData<&'a ()>,
	_no_sync: PhantomNoSync,
	gc_guard: RwLockReadGuard<'static, ()>,
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
		let gc_guard = object_db().gc_lock.read();

		// Acquire the session data.
		let mut sess_data = object_db().sess_data.lock();

		// Register a session ID.
		let id = sess_data.free_sessions.alloc();
		assert!(
			id != 255,
			"Cannot create mroe than 256 concurrent sessions!"
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
	slot: &'static SlotData,
	gen: ExtendedGen,
	meta: <T as Pointee>::Metadata,
}

impl<T: Sized + ObjPointee + Sync> Obj<T> {
	pub fn new(value: T) -> Self {
		todo!()
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	fn new_in_raw(session: &Session, lock: u8, value: T) -> Self {
		todo!()
	}
}

impl<T: ?Sized + ObjPointee> Eq for Obj<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for Obj<T> {
	fn eq(&self, other: &Self) -> bool {
		self.slot as *const SlotData == other.slot as *const SlotData && self.gen == other.gen
	}
}

impl<T: ?Sized + ObjPointee> Hash for Obj<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		(self.slot as *const SlotData).hash(state);
		self.gen.hash(state);
	}
}
