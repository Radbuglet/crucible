use std::{
	any::{type_name, Any},
	fmt,
};

use crate::lang::lifetime::try_transform_mut;

// === Userdata === //

pub type Userdata = Box<dyn UserdataValue>;

pub trait UserdataValue: 'static + fmt::Debug + Send + Sync {
	#[doc(hidden)]
	fn internal_as_any(&self) -> &(dyn Any + Send + Sync);

	#[doc(hidden)]
	fn internal_as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

	#[doc(hidden)]
	fn internal_type_name(&self) -> &'static str;
}

impl<T: fmt::Debug + Any + Send + Sync> UserdataValue for T {
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

impl dyn UserdataValue {
	pub fn type_name(&self) -> &'static str {
		self.internal_type_name()
	}

	pub fn try_downcast_ref<T: Any>(&self) -> Option<&T> {
		self.internal_as_any().downcast_ref::<T>()
	}

	pub fn try_downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
		self.internal_as_any_mut().downcast_mut::<T>()
	}

	fn unexpected_userdata<T>(&self) -> ! {
		panic!(
			"Expected userdata of type {}, got userdata of type {:?}.",
			type_name::<T>(),
			self.internal_type_name(),
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
