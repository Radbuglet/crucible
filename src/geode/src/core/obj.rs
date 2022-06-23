use std::{
	alloc::Layout,
	borrow::Cow,
	cell::{Ref, RefCell, RefMut},
	fmt, hash,
	marker::Unsize,
	num::NonZeroU64,
	ptr::{self, NonNull, Pointee},
	sync::atomic::AtomicU64,
};

use arr_macro::arr;
use derive_where::derive_where;
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, MutexGuard};
use thiserror::Error;

use crate::util::{
	cell::MutexedUnsafeCell,
	error::ResultExt,
	number::{LocalBatchAllocator, U8Alloc},
};

use super::{
	debug::{DebugLabel, NoLabel},
	internals::{
		gen::{ExtendedGen, SessionLocks, MAX_OBJ_GEN_EXCLUSIVE},
		heap::{GcHeap, Slot, SlotManager},
	},
	session::{Session, SessionStorage},
};

// === Singleton data === //

const ID_GEN_BATCH_SIZE: u64 = 4096;

struct GlobalData {
	id_batch_gen: AtomicU64,
	sess_data: OnceCell<Mutex<GlobalSessData>>,
}

struct GlobalSessData {
	lock_alloc: U8Alloc,
	lock_names: Box<[Option<Cow<'static, str>>; 256]>,
}

static GLOBAL_DATA: GlobalData = GlobalData {
	id_batch_gen: AtomicU64::new(1),
	sess_data: OnceCell::new(),
};

fn global_sess_data() -> MutexGuard<'static, GlobalSessData> {
	GLOBAL_DATA
		.sess_data
		.get_or_init(|| {
			Mutex::new(GlobalSessData {
				lock_alloc: U8Alloc::new(),
				lock_names: Box::new(arr![None; 256]),
			})
		})
		.lock()
}

static SESSION_DATA: SessionStorage<MutexedUnsafeCell<SessionData>> = SessionStorage::new();

#[derive(Default)]
struct SessionData {
	heap: GcHeap,
	slots: SlotManager,
	locks: SessionLocks,
	generation_gen: LocalBatchAllocator,
}

// === Session Extension === //

impl<'d> Session<'d> {
	pub fn new<I>(locks: I) -> Self
	where
		I: IntoIterator<Item = &'d mut LockToken>,
	{
		let mut session = Session::new_raw();
		let sess_data = SESSION_DATA.get_mut_or_init(&mut session).get_mut();
		sess_data.locks = SessionLocks::default();

		for lock in locks {
			sess_data.locks.lock(lock.handle().0);
		}

		session
	}
}

// === Locks === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(u8);

impl fmt::Debug for Lock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lock")
			.field("id", &self.0)
			.field(
				"debug_name",
				&global_sess_data().lock_names[self.0 as usize],
			)
			.finish()
	}
}

impl Lock {
	pub fn raw(self) -> u8 {
		self.0
	}
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LockToken(Lock);

impl Default for LockToken {
	fn default() -> Self {
		Self::new(NoLabel).0
	}
}

impl LockToken {
	pub fn new<L: DebugLabel>(label: L) -> (Self, Lock) {
		let mut global = global_sess_data();

		// Allocate ID
		let id = global.lock_alloc.alloc();
		assert_ne!(
			id, 0xFF,
			"Cannot allocate more than 255 locks continuously."
		);

		// Set debug name
		global.lock_names[id as usize] = label.to_debug_label();

		// Produce wrappers
		let id = Lock(id);
		(LockToken(id), id)
	}

	pub fn handle(&self) -> Lock {
		self.0
	}

	pub fn debug_name(&self) -> Option<Cow<'static, str>> {
		match global_sess_data().lock_names[self.handle().0 as usize].as_ref()? {
			Cow::Owned(instance) => Some(Cow::Owned(instance.clone())),
			Cow::Borrowed(instance) => Some(Cow::Borrowed(instance)),
		}
	}
}

impl Drop for LockToken {
	fn drop(&mut self) {
		let mut global = global_sess_data();

		global.lock_names[self.handle().0 as usize] = None;
		global.lock_alloc.free(self.handle().0);
	}
}

// === Obj Errors === //

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
		session: &Session,
		lock: Option<Lock>,
		layout: Layout,
	) -> (Self, NonNull<u8>) {
		let sess_data = unsafe {
			// Safety: TODO
			SESSION_DATA
				.get(session)
				.unwrap_unchecked()
				.get_mut_unchecked()
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
		(Self { slot, gen }, p_data)
	}

	pub fn new_dynamic(session: &Session, layout: Layout) -> (Self, NonNull<u8>) {
		Self::new_dynamic_in(session, None, layout)
	}

	// Fetching
	pub fn try_get_ptr(&self, session: &Session) -> Result<*const (), ObjGetError> {
		let sess_data = unsafe {
			// Safety: TODO
			SESSION_DATA
				.get(session)
				.unwrap_unchecked()
				.get_mut_unchecked()
		};

		match self.slot.try_get_base(&sess_data.locks, self.gen) {
			Ok(ptr) => Ok(ptr),
			Err(slot_gen) => {
				let lock_id = slot_gen.meta();
				if !sess_data.locks.check_lock(lock_id) {
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
		let sess_data = unsafe {
			// Safety: TODO
			&mut *SESSION_DATA.get(session).unwrap_unchecked().get()
		};

		self.slot.release();
		sess_data.slots.unreserve(self.slot);
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

		let sess_data = unsafe {
			// Safety: TODO
			SESSION_DATA
				.get(session)
				.unwrap_unchecked()
				.get_mut_unchecked()
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
		Self {
			raw: RawObj { slot, gen },
			meta,
		}
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	pub fn try_get<'a>(&self, session: &'a Session) -> Result<&'a T, ObjGetError> {
		let base = self.raw.try_get_ptr(session)?;
		let ptr = ptr::from_raw_parts::<T>(base, self.meta);

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

	pub fn unsize<U>(&self) -> Obj<U>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		let ptr = ptr::from_raw_parts::<T>(ptr::null(), self.meta) as *const U;
		let (_, meta) = ptr.to_raw_parts();

		Obj {
			raw: self.raw,
			meta,
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
	fn box_obj(self, session: &Session) -> Obj<Self>
	where
		Self: Sync,
	{
		Obj::new(session, self)
	}

	fn box_obj_in(self, session: &Session, lock: Lock) -> Obj<Self> {
		Obj::new_in(session, lock, self)
	}

	fn box_obj_rw(self, session: &Session, lock: Lock) -> Obj<RefCell<Self>> {
		Obj::new_rw(session, lock, self)
	}
}

impl<T: Sized + ObjPointee> ObjCtorExt for T {}
