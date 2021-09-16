use std::hash::Hash;
use std::slice::Iter as SliceIter;

// === Core === //

pub trait EnumMeta: 'static + Sized + Copy + Eq + Hash {
	// TODO: Expand to support non-copy enums
	type Meta: 'static;

	fn values() -> &'static [(Self, Self::Meta)];
	fn values_iter() -> EnumMetaIter<Self> {
		EnumMetaIter::from_slice(Self::values())
	}
	fn meta(self) -> &'static Self::Meta;
}

#[derive(Clone)]
pub struct EnumMetaIter<T: EnumMeta> {
	iter: SliceIter<'static, (T, T::Meta)>,
}

impl<T: EnumMeta> EnumMetaIter<T> {
	pub fn from_slice(slice: &'static [(T, T::Meta)]) -> Self {
		Self { iter: slice.iter() }
	}

	fn map_item(entry: &(T, T::Meta)) -> (T, &T::Meta) {
		(entry.0, &entry.1)
	}
}

impl<T: EnumMeta> ExactSizeIterator for EnumMetaIter<T> {}
impl<T: EnumMeta> Iterator for EnumMetaIter<T> {
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
    pub enum $item_name {$(
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

        fn values() -> &'static [(Self, Self::Meta)] {
            &Self::ITEMS
        }

        fn meta(self) -> &'static Self::Meta {
            for (var, meta) in Self::values() {
                if self == *var {
                    return meta;
                }
            }
            unreachable!()
        }
    }
)*}

// === Serialization === //

pub trait EnumDiscriminant {
	type Discriminant;

	fn discriminant(&self) -> Self::Discriminant;
}

impl EnumDiscriminant for u8 {
	type Discriminant = u8;

	fn discriminant(&self) -> Self::Discriminant {
		*self
	}
}

pub trait EnumMetaDiscriminantExt: EnumMeta
where
	Self::Meta: EnumDiscriminant<Discriminant = Self::Discriminant>,
{
	type Discriminant: Sized + Hash + Eq;

	fn to_disc(&self) -> Self::Discriminant;
	fn try_from_disc(discriminant: Self::Discriminant) -> Option<Self>;
	fn from_disc(discriminant: Self::Discriminant) -> Self {
		Self::try_from_disc(discriminant).unwrap()
	}
}

impl<D, M, T> EnumMetaDiscriminantExt for T
where
	D: Sized + Copy + Hash + Eq,
	M: 'static + EnumDiscriminant<Discriminant = D>,
	T: EnumMeta<Meta = M>,
{
	type Discriminant = D;

	fn to_disc(&self) -> Self::Discriminant {
		self.meta().discriminant()
	}

	fn try_from_disc(discriminant: Self::Discriminant) -> Option<Self> {
		Self::values_iter().find_map(move |(var, meta)| {
			if meta.discriminant() == discriminant {
				Some(var)
			} else {
				None
			}
		})
	}
}
