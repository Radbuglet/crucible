use std::{
	alloc::Layout,
	fmt::{self, Write},
	hash,
	ptr::NonNull,
};

use crucible_core::error::{ErrorFormatExt, ResultExt};
use thiserror::Error;

use super::{
	internals::{db, gen::ExtendedGen, heap::Slot},
	lock::Lock,
	session::Session,
};

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

pub type DropFn<M> = unsafe fn(Session, *mut u8, *mut M);

pub trait ObjPointee: 'static + Send {}

impl<T: ?Sized + 'static + Send> ObjPointee for T {}

#[derive(Copy, Clone)]
pub struct RawObj {
	slot: &'static Slot,
	gen: ExtendedGen,
}

impl fmt::Debug for RawObj {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RawObj")
			.field("gen", &self.ptr_gen())
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
	pub fn new(session: Session, lock: Option<Lock>, layout: Layout) -> (Self, NonNull<u8>) {
		let (slot, gen, full_ptr) =
			db::allocate_new_obj(session, layout, lock.map_or(0xFF, |lock| lock.slot()));

		(Self { slot, gen }, full_ptr)
	}

	pub fn new_at(session: Session, lock: Option<Lock>, target: *mut u8) -> Self {
		let (slot, gen) =
			db::allocate_new_obj_custom(session, target, lock.map_or(0xFF, |lock| lock.slot()));

		Self { slot, gen }
	}

	// Fetching
	fn decode_error(session: Session, requested: RawObj, slot_gen: ExtendedGen) -> ObjGetError {
		let lock_id = slot_gen.meta();

		if !db::is_lock_held_by(session, lock_id) {
			return ObjGetError::Locked(ObjLockedError {
				requested,
				lock: Lock(lock_id),
			});
		}

		debug_assert_ne!(slot_gen.gen(), requested.gen.gen());
		ObjGetError::Dead(ObjDeadError {
			requested,
			new_gen: slot_gen.gen(),
		})
	}

	pub fn try_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjGetError> {
		#[cold]
		#[inline(never)]
		fn decode_error(session: Session, requested: RawObj, slot_gen: ExtendedGen) -> ObjGetError {
			RawObj::decode_error(session, requested, slot_gen)
		}

		match db::try_get_obj_ptr(session, self.slot, self.gen) {
			Ok(ptr) => Ok(unsafe {
				// Safety: `RawObj` never points to a null pointer while alive.
				NonNull::new_unchecked(ptr.cast::<u8>())
			}),
			Err(slot_gen) => Err(decode_error(session, *self, slot_gen)),
		}
	}

	pub fn get_ptr(&self, session: Session) -> NonNull<u8> {
		// N.B. we don't `.unwrap_pretty()` on `try_get_ptr` because we want the entire cold path
		// to be in its own function to reduce codegen output size.
		#[cold]
		#[inline(never)]
		fn raise_error(session: Session, requested: RawObj, slot_gen: ExtendedGen) -> ! {
			RawObj::decode_error(session, requested, slot_gen).raise()
		}

		match db::try_get_obj_ptr(session, self.slot, self.gen) {
			Ok(ptr) => unsafe {
				// Safety: `RawObj` never points to a null pointer while alive.
				NonNull::new_unchecked(ptr.cast::<u8>())
			},
			Err(slot_gen) => raise_error(session, *self, slot_gen),
		}
	}

	pub fn weak_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get_ptr(session))
	}

	pub fn force_get_ptr(&self, session: Session) -> NonNull<u8> {
		// TODO: Elide checks in release builds.
		self.get_ptr(session)
	}

	// Lifecycle management
	pub fn is_alive_now(&self, _session: Session) -> bool {
		self.slot.is_alive(self.gen)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.gen.gen()
	}

	pub unsafe fn destroy<M>(
		&self,
		session: Session,
		drop_fn: Option<DropFn<M>>,
		meta: *mut M,
	) -> bool {
		db::destroy_obj(session, self.slot, self.gen)
	}
}
