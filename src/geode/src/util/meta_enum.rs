use crate::util::error::OkUnwrapExt;
use std::hash::Hash;
use std::slice::Iter as SliceIter;

pub trait EnumMeta: 'static + Sized + Copy + Eq + Hash {
	type Meta: 'static;

	fn index(self) -> usize;

	fn values() -> &'static [(Self, Self::Meta)];

	fn try_from_index(index: usize) -> Option<Self> {
		Self::values().get(index).map(|(variant, _)| *variant)
	}

	fn from_index(index: usize) -> Self {
		Self::try_from_index(index).unwrap_or_panic(|_| {
			format!(
				"Unknown variant {} of {}.",
				index,
				std::any::type_name::<Self>()
			)
		})
	}

	fn meta(self) -> &'static Self::Meta {
		&Self::values()[self.index()].1
	}

	fn values_iter() -> EnumPairIter<Self> {
		EnumPairIter::from_slice(Self::values())
	}

	fn variants() -> EnumVariantIter<Self> {
		EnumVariantIter::from_slice(Self::values())
	}

	fn find_where<F>(mut predicate: F) -> Option<Self>
	where
		F: FnMut(Self, &'static Self::Meta) -> bool,
	{
		Self::values_iter().find_map(|(val, meta)| {
			if predicate(val, meta) {
				Some(val)
			} else {
				None
			}
		})
	}
}

#[derive(Clone)]
pub struct EnumPairIter<T: EnumMeta> {
	iter: SliceIter<'static, (T, T::Meta)>,
}

impl<T: EnumMeta> EnumPairIter<T> {
	pub fn from_slice(slice: &'static [(T, T::Meta)]) -> Self {
		Self { iter: slice.iter() }
	}

	fn map_item(entry: &(T, T::Meta)) -> (T, &T::Meta) {
		(entry.0, &entry.1)
	}
}

impl<T: EnumMeta> ExactSizeIterator for EnumPairIter<T> {}
impl<T: EnumMeta> Iterator for EnumPairIter<T> {
	type Item = (T, &'static T::Meta);

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next().map(Self::map_item)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.iter.len();
		(len, Some(len))
	}

	fn count(self) -> usize
	where
		Self: Sized,
	{
		self.iter.len()
	}

	fn last(self) -> Option<Self::Item>
	where
		Self: Sized,
	{
		self.iter.last().map(Self::map_item)
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.iter.nth(n).map(Self::map_item)
	}
}

#[derive(Clone)]
pub struct EnumVariantIter<T: EnumMeta> {
	iter: SliceIter<'static, (T, T::Meta)>,
}

impl<T: EnumMeta> EnumVariantIter<T> {
	pub fn from_slice(slice: &'static [(T, T::Meta)]) -> Self {
		Self { iter: slice.iter() }
	}

	fn map_item(entry: &(T, T::Meta)) -> T {
		entry.0
	}
}

impl<T: EnumMeta> ExactSizeIterator for EnumVariantIter<T> {}
impl<T: EnumMeta> Iterator for EnumVariantIter<T> {
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next().map(Self::map_item)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.iter.len();
		(len, Some(len))
	}

	fn count(self) -> usize
	where
		Self: Sized,
	{
		self.iter.len()
	}

	fn last(self) -> Option<Self::Item>
	where
		Self: Sized,
	{
		self.iter.last().map(Self::map_item)
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.iter.nth(n).map(Self::map_item)
	}
}

pub macro enum_meta($(
    $(#[$item_attr:meta])*
    $vis:vis enum($meta_ty:ty) $item_name:ident {
        $(
			$(#[$var_attr:meta])*  // Also accepts doc comments, which are transformed into attributes during tokenization.
			$var_name:ident = $meta:expr
		),*
		$(,)?
    }
)*) {$(
    $(#[$item_attr])*
    #[derive(Copy, Clone, Eq, PartialEq, Hash)]
    $vis enum $item_name {$(
        $(#[$var_attr])*
        $var_name
	),*}

    impl $item_name {
        const ITEMS: [(Self, $meta_ty); 0 $(+ { let _ = Self::$var_name; 1 })*] = [
            $((Self::$var_name, $meta)),*
        ];
    }

    impl EnumMeta for $item_name {
        type Meta = $meta_ty;

        fn index(self) -> usize {
            self as usize
        }

        fn values() -> &'static [(Self, Self::Meta)] {
            &Self::ITEMS
        }
    }
)*}
