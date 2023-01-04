use std::{
	any::{type_name, Any},
	borrow::{Borrow, BorrowMut},
	fmt,
	ops::{Deref, DerefMut},
};

use crate::lang::lifetime::try_transform_mut;

// === Userdata === //

pub type BoxedUserdata = Box<dyn Userdata>;

pub trait Userdata: 'static + fmt::Debug + Send + Sync {
	#[doc(hidden)]
	fn internal_as_any(&self) -> &(dyn Any + Send + Sync);

	#[doc(hidden)]
	fn internal_as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

	#[doc(hidden)]
	fn internal_type_name(&self) -> &'static str;
}

impl<T: fmt::Debug + Any + Send + Sync> Userdata for T {
	fn internal_as_any(&self) -> &(dyn Any + Send + Sync) {
		self
	}

	fn internal_as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
		self
	}

	fn internal_type_name(&self) -> &'static str {
		type_name::<T>()
	}
}

fn unexpected_userdata<T>(ty_name: &str) -> ! {
	panic!(
		"Expected userdata of type {}, got userdata of type {:?}.",
		type_name::<T>(),
		ty_name,
	)
}

pub trait ErasedUserdata: Userdata {
	fn type_name(&self) -> &'static str {
		self.internal_type_name()
	}

	fn try_downcast_ref<T: Any>(&self) -> Option<&T> {
		self.internal_as_any().downcast_ref::<T>()
	}

	fn try_downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
		self.internal_as_any_mut().downcast_mut::<T>()
	}

	fn downcast_ref<T: Any>(&self) -> &T {
		self.try_downcast_ref()
			.unwrap_or_else(|| unexpected_userdata::<T>(self.type_name()))
	}

	fn downcast_mut<T: Any>(&mut self) -> &mut T {
		match try_transform_mut(self, |val| val.try_downcast_mut::<T>()) {
			Ok(val) => val,
			Err(val) => unexpected_userdata::<T>(val.type_name()),
		}
	}
}

impl ErasedUserdata for dyn Userdata {}

// === DebugOpaque === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct DebugOpaque<T: ?Sized> {
	pub value: T,
}

impl<T: ?Sized> DebugOpaque<T> {
	pub const fn new(value: T) -> Self
	where
		T: Sized,
	{
		Self { value }
	}
}

impl<T: ?Sized> Deref for DebugOpaque<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T: ?Sized> DerefMut for DebugOpaque<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

impl<T: ?Sized> fmt::Debug for DebugOpaque<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let ty_name = format!("DebugOpaque<{}>", type_name::<T>());

		f.debug_struct(&ty_name).finish_non_exhaustive()
	}
}

impl<T> From<T> for DebugOpaque<T> {
	fn from(value: T) -> Self {
		Self { value }
	}
}

impl<T: ?Sized> Borrow<T> for DebugOpaque<T> {
	fn borrow(&self) -> &T {
		&self.value
	}
}

impl<T: ?Sized> BorrowMut<T> for DebugOpaque<T> {
	fn borrow_mut(&mut self) -> &mut T {
		&mut self.value
	}
}
