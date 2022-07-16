use core::{fmt, hash, ops::Index};

use crate::{array::boxed_arr_repeat_len, marker::PhantomInvariant};

// === `ExposesVariants` === //

pub type VariantIter<T> = std::iter::Copied<std::slice::Iter<'static, T>>;

pub trait ExposesVariants: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq + Ord {
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
        $($field:ident),*
        $(,)?
    }
)*) {$(
    $(#[$attr_meta])*
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
    $vis enum $name {
        $($field),*
    }

    impl ExposesVariants for $name {
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
pub struct CEnumMap<K: ExposesVariants, V> {
	_ty: PhantomInvariant<K>,
	map: Box<[Option<V>]>,
}

impl<K: ExposesVariants, V> Default for CEnumMap<K, V> {
	fn default() -> Self {
		Self {
			_ty: Default::default(),
			map: boxed_arr_repeat_len(|| None, K::COUNT),
		}
	}
}

impl<K: ExposesVariants, V> CEnumMap<K, V> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert(&mut self, key: K, value: V) -> Option<V> {
		self.map[key.index()].replace(value)
	}

	pub fn get(&self, key: K) -> Option<&V> {
		self.map[key.index()].as_ref()
	}

	pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
		self.map[key.index()].as_mut()
	}

	pub fn values(&self) -> impl Iterator<Item = (K, &V)> + '_ {
		K::variants()
			.zip(self.map.iter())
			.filter_map(|(k, v)| Some((k, v.as_ref()?)))
	}

	pub fn values_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + '_ {
		K::variants()
			.zip(self.map.iter_mut())
			.filter_map(|(k, v)| Some((k, v.as_mut()?)))
	}
}

impl<K: ExposesVariants, V> Index<K> for CEnumMap<K, V> {
	type Output = V;

	fn index(&self, index: K) -> &Self::Output {
		self.get(index).unwrap()
	}
}
