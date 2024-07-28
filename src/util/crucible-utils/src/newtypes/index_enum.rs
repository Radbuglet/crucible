use std::{
    fmt, iter,
    ops::{Deref, DerefMut},
    slice,
};

use crate::traits::ArrayLike;

use super::{iterator, Index, IndexSlice, IndexSliceIter, IndexSliceIterMut};

// === Traits === //

#[derive(Debug, Clone)]
#[iterator(T, &mut self.0)]
pub struct EnumIndexVariants<T: EnumIndex>(iter::Copied<slice::Iter<'static, T>>);

pub trait EnumIndex: Index {
    const COUNT: usize = Self::VARIANTS.len();
    const VARIANTS: &'static [Self];

    type Array<T>: ArrayLike<Elem = T>;
    type BitSet: ArrayLike<Elem = Self::BitSetElem>;
    type BitSetElem: num_traits::PrimInt;

    fn variants() -> EnumIndexVariants<Self> {
        EnumIndexVariants(Self::VARIANTS.iter().copied())
    }
}

#[doc(hidden)]
pub mod enum_index_internals {
    use std::mem;
    pub use {
        super::{
            super::{Index, IndexOptions},
            EnumIndex,
        },
        std::{option::Option, primitive::usize},
    };

    pub const ENUM_INDEX_OPTIONS: IndexOptions = IndexOptions { use_map_fmt: true };

    pub trait TyIndex<const N: usize> {
        type Out;
    }

    impl TyIndex<0> for () {
        type Out = u8;
    }

    impl TyIndex<1> for () {
        type Out = u16;
    }

    impl TyIndex<2> for () {
        type Out = u32;
    }

    impl TyIndex<4> for () {
        type Out = u64;
    }

    pub const fn get_ideal_size(bits: usize) -> usize {
        let bytes = (bits + 7) / 8;

        match bytes {
            0 | 1 => 0, // u8
            2 => 1,     // u16
            3 | 4 => 2, // u32
            _ => 4,     // u64
        }
    }

    pub const fn get_elem_count<T>(bits: usize) -> usize {
        let word_size = 8 * mem::size_of::<T>();
        (bits + word_size - 1) / word_size
    }
}

#[macro_export]
macro_rules! enum_index {
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

        impl $crate::newtypes::enum_index_internals::Index for $name {
            const OPTIONS: $crate::newtypes::enum_index_internals::IndexOptions =
                $crate::newtypes::enum_index_internals::ENUM_INDEX_OPTIONS;

            fn try_from_usize(v: $crate::newtypes::enum_index_internals::usize) -> $crate::newtypes::enum_index_internals::Option<Self> {
                <Self as $crate::newtypes::enum_index_internals::EnumIndex>::VARIANTS.get(v).copied()
            }

            fn as_usize(self) -> $crate::newtypes::enum_index_internals::usize {
				self as $crate::newtypes::enum_index_internals::usize
			}
        }

		impl $crate::newtypes::enum_index_internals::EnumIndex for $name {
			const VARIANTS: &'static [Self] = &[
				$(Self::$field),*
			];

			type Array<T> = [T; Self::COUNT];

            type BitSetElem = <() as $crate::newtypes::enum_index_internals::TyIndex<{
                $crate::newtypes::enum_index_internals::get_ideal_size(Self::COUNT)
            }>>::Out;

            type BitSet = [
                Self::BitSetElem;
                {$crate::newtypes::enum_index_internals::get_elem_count::<Self::BitSetElem>(Self::COUNT)}
            ];
		}
	)*};
}

pub use enum_index;

// === IndexArray === //

#[iterator(V, &mut self.0)]
pub struct IndexArrayIntoIter<K: EnumIndex, V>(<K::Array<V> as IntoIterator>::IntoIter);

#[iterator((K, V), &mut self.0)]
pub struct IndexArrayIntoEnumerate<K: EnumIndex, V>(
    iter::Zip<EnumIndexVariants<K>, IndexArrayIntoIter<K, V>>,
);

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct IndexArray<K: EnumIndex, V> {
    pub raw: K::Array<V>,
}

impl<K: EnumIndex, V: fmt::Debug> fmt::Debug for IndexArray<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.enumerate()).finish()
    }
}

impl<K: EnumIndex, V: Default> Default for IndexArray<K, V> {
    fn default() -> Self {
        Self {
            raw: <K::Array<V>>::from_fn(|_| V::default()),
        }
    }
}

impl<K: EnumIndex, V> IndexArray<K, V> {
    pub const fn new(values: K::Array<V>) -> Self {
        Self { raw: values }
    }

    pub fn into_enumerate(self) -> IndexArrayIntoEnumerate<K, V> {
        IndexArrayIntoEnumerate(K::variants().zip(self))
    }
}

impl<K: EnumIndex, V> Deref for IndexArray<K, V> {
    type Target = IndexSlice<K, V>;

    fn deref(&self) -> &Self::Target {
        IndexSlice::from_raw_ref(self.raw.as_ref())
    }
}

impl<K: EnumIndex, V> DerefMut for IndexArray<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        IndexSlice::from_raw_mut(self.raw.as_mut())
    }
}

impl<'a, K: EnumIndex, V> IntoIterator for &'a IndexArray<K, V> {
    type IntoIter = IndexSliceIter<'a, V>;
    type Item = &'a V;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: EnumIndex, V> IntoIterator for &'a mut IndexArray<K, V> {
    type IntoIter = IndexSliceIterMut<'a, V>;
    type Item = &'a mut V;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K: EnumIndex, V> IntoIterator for IndexArray<K, V> {
    type IntoIter = IndexArrayIntoIter<K, V>;
    type Item = V;

    fn into_iter(self) -> Self::IntoIter {
        IndexArrayIntoIter(self.raw.into_iter())
    }
}

impl<K: EnumIndex, V> FromIterator<V> for IndexArray<K, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self::new(K::Array::<V>::from_iter(iter))
    }
}
