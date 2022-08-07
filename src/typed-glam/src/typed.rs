use bytemuck::TransparentWrapper;
use crucible_core::std_traits::ArrayLike;

use std::{any::type_name, fmt, hash};

use crate::traits::{IntegerVector, NumericVector};

pub trait VecFlavor {
	type Backing: NumericVector;

	const NAME: &'static str;
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
pub struct TypedVector<F: ?Sized + VecFlavor>(F::Backing);

// NumericVector
impl<F: ?Sized + VecFlavor> fmt::Debug for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple(format!("TypedVector<{}>", type_name::<F>()).as_str())
			.field(&self.0)
			.finish()
	}
}

impl<F: ?Sized + VecFlavor> fmt::Display for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}({:?})", F::NAME, self.0.to_array().as_slice())
	}
}

impl<F: ?Sized + VecFlavor> Copy for TypedVector<F> {}

impl<F: ?Sized + VecFlavor> Clone for TypedVector<F> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<F: ?Sized + VecFlavor> PartialEq for TypedVector<F> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl<F: ?Sized + VecFlavor> Default for TypedVector<F> {
	fn default() -> Self {
		Self(Default::default())
	}
}

// IntegerVector
impl<F: ?Sized + VecFlavor> hash::Hash for TypedVector<F>
where
	F::Backing: IntegerVector,
{
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}

impl<F: ?Sized + VecFlavor> Eq for TypedVector<F> where F::Backing: IntegerVector {}
