use std::{
	alloc::Layout,
	cell::{Ref, RefCell, RefMut},
	fmt::{self, Write},
	hash,
	marker::Unsize,
	ptr::Pointee,
};

use thiserror::Error;

use crate::util::{error::ResultExt, ptr::unsize_meta};

use super::{
	debug::DebugLabel,
	internals::{db, gen::ExtendedGen, heap::Slot},
	owned::{Destructible, Owned},
	reflect::ReflectType,
	session::{LocalSessionGuard, Session},
};

// === Locks === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(u8);

impl Lock {
	pub fn new<L: DebugLabel>(label: L) -> Owned<Self> {
		let id = db::reserve_lock(label.to_debug_label());
		Owned::new(Lock(id))
	}

	pub fn is_held(self) -> bool {
		db::is_lock_held_somewhere(self.slot())
	}

	pub fn slot(self) -> u8 {
		self.0
	}
}

impl Destructible for Lock {
	fn destruct(self) {
		db::unreserve_lock(self.slot())
	}
}

impl fmt::Debug for Lock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lock")
			.field("slot", &self.slot())
			.field("debug_name", &db::get_lock_debug_name(self.slot()))
			.finish()
	}
}

// === Session extensions === //

impl Session<'_> {
	pub fn acquire_locks<I: IntoIterator<Item = Lock>>(self, locks: I) {
		db::acquire_locks(
			self,
			&locks
				.into_iter()
				.map(|lock| lock.slot())
				.collect::<Vec<_>>(),
		);
	}

	pub fn reserve_slot_capacity(self, amount: usize) {
		db::reserve_obj_slot_capacity(self, amount)
	}
}

// === Obj Errors === //

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to fetch `Obj`")]
pub enum ObjGetError {
	Dead(#[from] ObjDeadError),
	Locked(#[from] ObjLockedError),
}

impl ObjGetError {
	pub fn as_lifetime_error(self) -> Result<ObjDeadError, ObjLockedError> {
		match self {
			Self::Dead(value) => Ok(value),
			Self::Locked(locked) => Err(locked),
		}
	}

	pub fn unwrap_weak<T>(result: Result<T, Self>) -> Result<T, ObjDeadError> {
		result.map_err(|err| err.as_lifetime_error().unwrap_pretty())
	}
}

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
	) -> (Owned<Self>, *mut ()) {
		let (slot, gen, initial_ptr) = db::allocate_new_obj(
			session,
			ReflectType::dynamic_no_drop(),
			layout,
			lock.map_or(0xFF, |lock| lock.slot()),
		);

		(Owned::new(Self { slot, gen }), initial_ptr)
	}

	pub fn new_dynamic(session: Session, layout: Layout) -> (Owned<Self>, *mut ()) {
		Self::new_dynamic_in(session, None, layout)
	}

	// Fetching
	pub fn try_get_ptr(&self, session: Session) -> Result<*const (), ObjGetError> {
		#[cold]
		#[inline(never)]
		fn decode_error(session: Session, requested: RawObj, slot_gen: ExtendedGen) -> ObjGetError {
			let lock_id = slot_gen.meta();

			if db::is_lock_held_by(session, lock_id) {
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

		match db::try_get_obj_ptr(session, self.slot, self.gen) {
			Ok(ptr) => Ok(ptr),
			Err(slot_gen) => Err(decode_error(session, *self, slot_gen)),
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

	pub fn destroy(&self, session: Session) -> bool {
		db::destroy_obj(session, self.slot, self.gen)
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

pub struct Obj<T: ?Sized + ObjPointee> {
	raw: RawObj,
	meta: <T as Pointee>::Metadata,
}

impl<T: Sized + ObjPointee + Sync> Obj<T> {
	#[inline(always)]
	pub fn new(session: Session, value: T) -> Owned<Self> {
		Self::new_in_raw(session, 0xFF, value)
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	#[inline(always)]
	pub fn new_in(session: Session, lock: Lock, value: T) -> Owned<Self> {
		Self::new_in_raw(session, lock.0, value)
	}

	#[inline(always)]
	fn new_in_raw(session: Session, lock: u8, value: T) -> Owned<Self> {
		// Allocate slot
		let (slot, gen, initial_ptr) =
			db::allocate_new_obj(session, ReflectType::of::<T>(), Layout::new::<T>(), lock);

		// Write initial data
		let initial_ptr = initial_ptr.cast::<T>();

		unsafe {
			initial_ptr.write(value);
		}

		// Obtain pointer metadata (should always be `()` but we do this anyways because `T: Sized`
		// does not imply `<T as Pointee>::Metadata == ()` to the type checker yet)
		let (_, meta) = initial_ptr.to_raw_parts();

		Owned::new(Self {
			raw: RawObj { slot, gen },
			meta,
		})
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	pub fn try_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjGetError> {
		let base_addr = self.raw.try_get_ptr(session)?;
		let ptr = std::ptr::from_raw_parts(base_addr, self.meta);

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

	pub fn destroy<'a>(&self, session: Session<'a>) -> bool {
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

impl<T: ?Sized + ObjPointee> Copy for Obj<T> {}

impl<T: ?Sized + ObjPointee> Clone for Obj<T> {
	fn clone(&self) -> Self {
		Self {
			raw: self.raw,
			meta: self.meta,
		}
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
		Owned::new(self.manually_destruct().as_unsized::<U>())
	}

	pub fn to_raw(self) -> Owned<RawObj> {
		Owned::new(self.manually_destruct().as_raw())
	}
}
