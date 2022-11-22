use std::{
	any::{type_name, Any},
	fmt,
	ops::Deref,
	sync::Arc,
};

use derive_where::derive_where;

use crate::lang::lifetime::try_transform_mut;

// === Userdata === //

pub type Userdata = Box<dyn UserdataValue>;

pub trait UserdataValue: fmt::Debug + Any + Send + Sync {
	#[doc(hidden)]
	fn inner_as_any(&self) -> &(dyn Any + Send + Sync);

	#[doc(hidden)]
	fn inner_as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

	#[doc(hidden)]
	fn inner_type_name(&self) -> &'static str;
}

impl<T: fmt::Debug + Any + Send + Sync> UserdataValue for T {
	fn inner_as_any(&self) -> &(dyn Any + Send + Sync) {
		self
	}

	fn inner_as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
		self
	}

	fn inner_type_name(&self) -> &'static str {
		type_name::<T>()
	}
}

impl dyn UserdataValue {
	pub fn type_name(&self) -> &'static str {
		self.inner_type_name()
	}

	pub fn try_downcast_ref<T: Any>(&self) -> Option<&T> {
		self.inner_as_any().downcast_ref::<T>()
	}

	pub fn try_downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
		self.inner_as_any_mut().downcast_mut::<T>()
	}

	fn unexpected_userdata<T>(&self) -> ! {
		panic!(
			"Expected userdata of type {}, got userdata of type {:?}.",
			type_name::<T>(),
			self.inner_type_name(),
		)
	}

	pub fn downcast_ref<T: Any>(&self) -> &T {
		self.try_downcast_ref()
			.unwrap_or_else(|| self.unexpected_userdata::<T>())
	}

	pub fn downcast_mut<T: Any>(&mut self) -> &mut T {
		match try_transform_mut(self, |val| val.try_downcast_mut::<T>()) {
			Ok(val) => val,
			Err(val) => val.unexpected_userdata::<T>(),
		}
	}
}

// === UserdataArcRef === //

#[derive(Debug)]
#[derive_where(Copy, Clone)]
pub struct UserdataArcRef<'a, T: 'static> {
	arc: &'a Arc<dyn UserdataValue>,
	val: &'a T,
}

impl<'a, T: 'static> UserdataArcRef<'a, T> {
	pub fn new(arc: &'a Arc<dyn UserdataValue>) -> Self {
		Self {
			arc,
			val: arc.downcast_ref(),
		}
	}
}

impl<'a, T> Into<Arc<T>> for UserdataArcRef<'a, T> {
	fn into(self) -> Arc<T> {
		let ptr = Arc::into_raw(self.arc.clone());

		// Safety: we already verified that `dyn Userdata` was actually `T` in the constructor.
		let ptr = ptr as *const T;
		unsafe { Arc::from_raw(ptr) }
	}
}

impl<'a, T> Deref for UserdataArcRef<'a, T> {
	type Target = &'a T; // This should allow users to borrow the value for its full duration.

	fn deref(&self) -> &Self::Target {
		&self.val
	}
}
