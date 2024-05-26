use std::{fmt, hash, iter, ops, slice};

use newtypes_proc::iterator;
use std_traits::ArrayLike;

// === NumEnum === //

#[derive(Debug, Clone)]
#[iterator(T, &mut self.0)]
pub struct NumEnumVariants<T: NumEnum>(iter::Copied<slice::Iter<'static, T>>);

pub trait NumEnum: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq + Ord {
    const COUNT: usize = Self::VARIANTS.len();
    const VARIANTS: &'static [Self];

    type Array<T>: ArrayLike<Elem = T>;

    fn index(self) -> usize;

    fn try_from_index(index: usize) -> Option<Self> {
        Self::VARIANTS.get(index).copied()
    }

    fn variants() -> NumEnumVariants<Self> {
        NumEnumVariants(Self::VARIANTS.iter().copied())
    }
}

#[doc(hidden)]
pub mod num_enum_internals {
    pub use {super::NumEnum, std::primitive::usize};
}

#[macro_export]
macro_rules! num_enum {
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

		impl $crate::num_enum_internals::NumEnum for $name {
			const VARIANTS: &'static [Self] = &[
				$(Self::$field),*
			];

			type Array<T> = [T; Self::COUNT];

			fn index(self) -> $crate::num_enum_internals::usize {
				self as $crate::num_enum_internals::usize
			}
		}
	)*};
}

// === NumEnumMap === //

#[derive(Debug, Clone)]
#[iterator(&'a V, &mut self.0)]
pub struct NumEnumMapValues<'a, V>(slice::Iter<'a, V>);

#[derive(Debug)]
#[iterator(&'a mut V, &mut self.0)]
pub struct NumEnumMapValuesMut<'a, V>(slice::IterMut<'a, V>);

#[iterator(V, &mut self.0)]
pub struct NumEnumMapIntoValues<K: NumEnum, V>(<K::Array<V> as IntoIterator>::IntoIter);

#[derive(Debug, Clone)]
#[iterator((K, &'a V), &mut self.0)]
pub struct NumEnumMapIter<'a, K: NumEnum, V>(
    iter::Zip<NumEnumVariants<K>, NumEnumMapValues<'a, V>>,
);

#[iterator((K, V), &mut self.0)]
pub struct NumEnumMapIntoIter<K: NumEnum, V>(
    iter::Zip<NumEnumVariants<K>, NumEnumMapIntoValues<K, V>>,
);

#[derive(Debug)]
#[iterator((K, &'a mut V), &mut self.0)]
pub struct NumEnumMapIterMut<'a, K: NumEnum, V>(
    iter::Zip<NumEnumVariants<K>, NumEnumMapValuesMut<'a, V>>,
);

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct NumEnumMap<K: NumEnum, V> {
    map: K::Array<V>,
}

impl<K: NumEnum, V: fmt::Debug> fmt::Debug for NumEnumMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: NumEnum, V: Default> Default for NumEnumMap<K, V> {
    fn default() -> Self {
        Self {
            map: <K::Array<V>>::from_fn(|_| V::default()),
        }
    }
}

impl<K: NumEnum, V> NumEnumMap<K, V> {
    pub const fn new(values: K::Array<V>) -> Self {
        Self { map: values }
    }

    pub fn keys(&self) -> NumEnumVariants<K> {
        K::variants()
    }

    pub fn values(&self) -> NumEnumMapValues<'_, V> {
        NumEnumMapValues(self.map.as_slice().iter())
    }

    pub fn values_mut(&mut self) -> NumEnumMapValuesMut<'_, V> {
        NumEnumMapValuesMut(self.map.as_slice_mut().iter_mut())
    }

    pub fn into_values(self) -> NumEnumMapIntoValues<K, V> {
        NumEnumMapIntoValues(self.map.into_iter())
    }

    pub fn iter(&self) -> NumEnumMapIter<'_, K, V> {
        NumEnumMapIter(K::variants().zip(self.values()))
    }

    pub fn iter_mut(&mut self) -> NumEnumMapIterMut<'_, K, V> {
        NumEnumMapIterMut(K::variants().zip(self.values_mut()))
    }
}

impl<K: NumEnum, V> IntoIterator for NumEnumMap<K, V> {
    type IntoIter = NumEnumMapIntoIter<K, V>;
    type Item = (K, V);

    fn into_iter(self) -> Self::IntoIter {
        NumEnumMapIntoIter(K::variants().zip(self.into_values()))
    }
}

impl<'a, K: NumEnum, V> IntoIterator for &'a NumEnumMap<K, V> {
    type IntoIter = NumEnumMapIter<'a, K, V>;

    type Item = (K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: NumEnum, V> IntoIterator for &'a mut NumEnumMap<K, V> {
    type IntoIter = NumEnumMapIterMut<'a, K, V>;

    type Item = (K, &'a mut V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K: NumEnum, V> FromIterator<V> for NumEnumMap<K, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self::new(K::Array::<V>::from_iter(iter))
    }
}

impl<K: NumEnum, V> ops::Index<K> for NumEnumMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.map[index.index()]
    }
}

impl<K: NumEnum, V> ops::IndexMut<K> for NumEnumMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.map[index.index()]
    }
}
