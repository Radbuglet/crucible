use std::{fmt, hash};

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
