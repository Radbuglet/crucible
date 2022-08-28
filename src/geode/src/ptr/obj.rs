use std::{
	fmt, hash,
	ptr::{self, Pointee},
};

use crucible_core::sync::MutexedPtr;

use crate::core::{
	lock::{Lock, LockAndMeta},
	object_db::{Slot, SlotDeadError},
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session},
};

pub trait ObjPointee: 'static + Send + Sync {}

impl<T: ?Sized + 'static + Send + Sync> ObjPointee for T {}

pub struct Obj<T: ?Sized + ObjPointee> {
	slot: Slot,
	gen: LockAndMeta,
	meta: <T as Pointee>::Metadata,
}

impl<T: ObjPointee + fmt::Debug> fmt::Debug for Obj<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let session = LocalSessionGuard::new();
		let s = session.handle();
		let err_keep_alive;

		f.debug_struct("Obj")
			.field("slot", &self.slot)
			.field("gen", &self.gen)
			.field(
				"value",
				match self.try_get(s) {
					Ok(val) => val as &dyn fmt::Debug,
					Err(err) => {
						err_keep_alive = err;
						&err_keep_alive
					}
				},
			)
			.finish()
	}
}

impl<T: ?Sized + ObjPointee> Copy for Obj<T> {}

impl<T: ?Sized + ObjPointee> Clone for Obj<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized + ObjPointee> hash::Hash for Obj<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.gen.hash(state);
	}
}

impl<T: ?Sized + ObjPointee> Eq for Obj<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for Obj<T> {
	fn eq(&self, other: &Self) -> bool {
		self.gen == other.gen
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	pub fn new(session: Session, value: T) -> Owned<Self>
	where
		T: Sized,
	{
		let val = Box::leak(Box::new(value)) as *mut T;
		let (base, meta) = val.to_raw_parts();
		let (slot, gen) = Slot::new(session, Lock::ALWAYS_MUTABLE, base);

		Owned::new(Self { slot, gen, meta })
	}

	pub fn try_get(self, _session: Session) -> Result<&T, SlotDeadError> {
		// N.B. we request a `_session` instance, despite not using it, to prevent the GC from
		// running until all references into GC'd contents expire.

		let base = self.slot.try_fetch_no_lock(self.gen)?;
		let ptr = ptr::from_raw_parts(base, self.meta);
		Ok(unsafe { &*ptr })
	}

	pub fn get(self, _session: Session) -> &T {
		// N.B. see `try_get`

		let base = self.slot.fetch_no_lock(self.gen);
		let ptr = ptr::from_raw_parts(base, self.meta);
		unsafe { &*ptr }
	}

	pub fn is_alive_now(self) -> bool {
		self.slot.try_fetch_no_lock(self.gen).is_ok()
	}

	pub fn slot(self) -> Slot {
		self.slot
	}

	pub fn gen_handle(self) -> LockAndMeta {
		self.gen
	}

	pub fn destroy(self, session: Session) -> Result<(), SlotDeadError> {
		let base = self.slot.try_destroy(session, self.gen)?;

		let ptr = ptr::from_raw_parts_mut::<T>(base, self.meta);

		unsafe {
			let ptr = MutexedPtr::from(ptr);

			session.add_gc_finalizer(move |_: Session| {
				// Safety: we allow unsizing coercions but we never allow these coercions to change the
				// layout of the object.
				drop(Box::from_raw(ptr.ptr()));
			});
		}

		Ok(())
	}
}

impl<T: ?Sized + ObjPointee> Destructible for Obj<T> {
	fn destruct(self) {
		let _ = self.destroy(LocalSessionGuard::new().handle());
	}
}
