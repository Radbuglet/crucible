use core::any::Any;

pub trait AnyEq {
	fn eq_other(&self, other: &dyn Any) -> Option<bool>;
}

impl<T: 'static + Eq> AnyEq for T {
	fn eq_other(&self, other: &dyn Any) -> Option<bool> {
		Some(self == other.downcast_ref::<T>()?)
	}
}
