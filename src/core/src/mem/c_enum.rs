use std::{fmt, hash, marker::PhantomData, ops::Index, ops::IndexMut};

use crate::lang::{marker::PhantomInvariant, std_traits::ArrayLike};

// === `ExposesVariants` === //

pub type VariantIter<T> = std::iter::Copied<std::slice::Iter<'static, T>>;

pub trait CEnum: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq + Ord {
	const COUNT: usize = Self::VARIANTS.len();
	const VARIANTS: &'static [Self];

	type Array<T>: ArrayLike<Elem = T>;

	fn new_array<T, F>(gen: F) -> Self::Array<T>
	where
		F: FnMut(usize) -> T;

	fn index(self) -> usize;

	fn try_from_index(index: usize) -> Option<Self> {
		Self::VARIANTS.get(index).copied()
	}

	fn variants() -> VariantIter<Self> {
		Self::VARIANTS.iter().copied()
	}
}

#[doc(hidden)]
pub mod macro_internal {
	pub use std::primitive::usize;
}

#[macro_export]
macro_rules! c_enum {
	($(
		$(#[$attr_meta:meta])*
		$vis:vis enum $name:ident {
			$(
				$(#[$field_meta:meta])*
				$field:ident
			),*
			$(,)?
		}
	)*) => {$(
		$(#[$attr_meta])*
		#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
		$vis enum $name {
			$(
				$(#[$field_meta])*
				$field
			),*
		}

		impl $crate::mem::c_enum::CEnum for $name {
			const VARIANTS: &'static [Self] = &[
				$(Self::$field),*
			];

			type Array<T> = [T; Self::COUNT];

			fn new_array<T, F>(mut gen: F) -> Self::Array<T>
			where
				F: ::std::ops::FnMut(usize) -> T,
			{
				$crate::arr![i => gen(i); Self::COUNT]
			}


			fn index(self) -> $crate::mem::c_enum::macro_internal::usize {
				self as $crate::mem::c_enum::macro_internal::usize
			}
		}
	)*};
}

pub use c_enum;

// === `CEnumMap` === //

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CEnumMap<K: CEnum, V> {
	_ty: PhantomInvariant<K>,
	map: K::Array<V>,
}

impl<K: CEnum, V: Default> Default for CEnumMap<K, V> {
	fn default() -> Self {
		Self {
			_ty: Default::default(),
			map: K::new_array(|_| V::default()),
		}
	}
}

impl<K: CEnum, V> CEnumMap<K, V> {
	pub fn new(values: K::Array<V>) -> Self {
		Self {
			_ty: PhantomData,
			map: values,
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
		K::variants().zip(self.map.as_slice().iter())
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + '_ {
		K::variants().zip(self.map.as_slice_mut().iter_mut())
	}

	pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
		self.iter().map(|(_, v)| v)
	}

	pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> + '_ {
		self.iter_mut().map(|(_, v)| v)
	}
}

impl<K: CEnum, V> Index<K> for CEnumMap<K, V> {
	type Output = V;

	fn index(&self, index: K) -> &Self::Output {
		&self.map[index.index()]
	}
}

impl<K: CEnum, V> IndexMut<K> for CEnumMap<K, V> {
	fn index_mut(&mut self, index: K) -> &mut Self::Output {
		&mut self.map[index.index()]
	}
}
