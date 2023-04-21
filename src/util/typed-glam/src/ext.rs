use crate::traits::{DimClass, NumericVector};
use crucible_util::lang::std_traits::ArrayLike;

pub trait VecExt: NumericVector {
	fn dim() -> usize {
		<Self::Dim as DimClass>::DIM
	}

	fn comps(&self) -> VecCompIter<'_, Self> {
		VecCompIter { vec: self, idx: 0 }
	}

	fn map<F>(self, f: F) -> Self
	where
		F: FnMut(Self::Comp) -> Self::Comp,
	{
		Self::from_array(Self::CompArray::from_iter(self.comps().map(f)))
	}

	fn all<F>(self, f: F) -> bool
	where
		F: FnMut(Self::Comp) -> bool,
	{
		self.comps().all(f)
	}
}

impl<V: NumericVector> VecExt for V {}

pub struct VecCompIter<'a, V> {
	vec: &'a V,
	idx: usize,
}

impl<V: NumericVector> Iterator for VecCompIter<'_, V> {
	type Item = V::Comp;

	fn next(&mut self) -> Option<Self::Item> {
		if self.idx < V::dim() {
			let comp = self.vec[self.idx];
			self.idx += 1;
			Some(comp)
		} else {
			None
		}
	}
}
