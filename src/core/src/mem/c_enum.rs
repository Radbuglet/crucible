use std::{fmt, hash, marker::PhantomData, ops::Index, ops::IndexMut};

use super::array::boxed_arr_from_fn;
use crate::lang::marker::PhantomInvariant;

// === `ExposesVariants` === //

pub type VariantIter<T> = std::iter::Copied<std::slice::Iter<'static, T>>;

pub trait CEnum: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq + Ord {
	const COUNT: usize = Self::VARIANTS.len();
	const VARIANTS: &'static [Self];

	fn index(self) -> usize;

	fn try_from_index(index: usize) -> Option<Self> {
		Self::VARIANTS.get(index).copied()
	}

	fn variants() -> VariantIter<Self> {
		Self::VARIANTS.iter().copied()
	}
}

pub macro c_enum($(
    $(#[$attr_meta:meta])*
    $vis:vis enum $name:ident {
        $(
			$(#[$field_meta:meta])*
			$field:ident
		),*
        $(,)?
    }
)*) {$(
    $(#[$attr_meta])*
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
    $vis enum $name {
        $(
			$(#[$field_meta])*
			$field
		),*
    }

    impl CEnum for $name {
        const VARIANTS: &'static [Self] = &[
            $(Self::$field),*
        ];

        fn index(self) -> usize {
            self as usize
        }
    }
)*}

// === `CEnumMap` === //

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CEnumMap<K: CEnum, V> {
	_ty: PhantomInvariant<K>,
	// TODO: Use a statically sized array once generic consts stabilize
	map: Box<[V]>,
}

impl<K: CEnum, V: Default> Default for CEnumMap<K, V> {
	fn default() -> Self {
		Self {
			_ty: Default::default(),
			map: boxed_arr_from_fn(Default::default, K::COUNT),
		}
	}
}

impl<K: CEnum, V> CEnumMap<K, V> {
	pub fn new<const N: usize>(values: [V; N]) -> Self {
		assert_eq!(values.len(), K::COUNT);

		Self {
			_ty: PhantomData,
			map: Box::new(values),
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
		K::variants().zip(self.map.iter())
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + '_ {
		K::variants().zip(self.map.iter_mut())
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
