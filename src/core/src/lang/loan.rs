use std::{fmt, ops::Deref, sync::Arc};

use parking_lot::{RwLock, RwLockReadGuard};

use crate::{
	debug::userdata::{ErasedUserdataValue, UserdataValue},
	mem::{
		drop_guard::{DropGuard, DropGuardHandler},
		ptr::{addr_of_ptr, PointeeCastExt},
	},
};

// Arc<T> => Arc<U>
pub fn map_arc<T: ?Sized, U: ?Sized, F>(arc: Arc<T>, f: F) -> Arc<U>
where
	F: FnOnce(&T) -> &U,
{
	let ptr = Arc::into_raw(arc);
	let converted = f(unsafe { &*ptr }) as *const U;
	assert_eq!(addr_of_ptr(ptr), addr_of_ptr(converted));

	unsafe {
		// Safety: `f` gives a proof that it can convert a reference of `&'a T` into a reference of
		// `&'a U`. Additionally, because the pointer address is the same, we know that we're pointing
		// to a valid `Arc`.
		Arc::from_raw(converted)
	}
}

pub fn downcast_userdata_arc<T: UserdataValue>(arc: Arc<dyn UserdataValue>) -> Arc<T> {
	map_arc(arc, |val| val.downcast_ref::<T>())
}

// Arc<RwLock<T>> => Arc<RwLockGuard<T>>
pub struct ArcReadGuard<T: ?Sized + 'static>(DropGuard<ArcReadGuardInner<T>, ArcReadGuardDtor>);

struct ArcReadGuardInner<T: ?Sized + 'static> {
	arc: Arc<RwLock<T>>,
	guard: RwLockReadGuard<'static, T>,
}

struct ArcReadGuardDtor;

impl<T: ?Sized + 'static> ArcReadGuard<T> {
	pub fn try_new(arc: Arc<RwLock<T>>) -> Option<Self> {
		let arc_val: &RwLock<T> = &*arc;
		let arc_val = unsafe { arc_val.prolong() };

		arc_val.try_read().map(|guard| {
			Self(DropGuard::new(
				ArcReadGuardInner { arc, guard },
				ArcReadGuardDtor,
			))
		})
	}

	pub fn into_arc(self) -> Arc<RwLock<T>> {
		let inner = DropGuard::defuse(self.0);

		// Release the guard.
		drop(inner.guard);

		// Give up the arc.
		inner.arc
	}
}

impl<T: ?Sized + 'static + fmt::Debug> fmt::Debug for ArcReadGuard<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let value: &T = &*self.0.guard;
		let value_dyn: &dyn fmt::Debug = &value; // We're actually calling `fmt` on a `&&T`. Such is life.

		f.debug_tuple("ArcReadGuard").field(value_dyn).finish()
	}
}

impl<T: ?Sized + 'static> Deref for ArcReadGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0.guard
	}
}

impl<T: ?Sized + 'static> DropGuardHandler<ArcReadGuardInner<T>> for ArcReadGuardDtor {
	fn destruct(self, value: ArcReadGuardInner<T>) {
		// First, we drop the guard to remove any remaining dependencies on the `Arc`.
		drop(value.guard);

		// Because no one else is looking at the contents of this `Arc`, we can drop it.
		drop(value.arc);
	}
}
