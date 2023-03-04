use std::{borrow::Borrow, hash};

// === Macros === //

#[macro_export]
macro_rules! impl_tuples {
	// Internal
	(
		$target:path : []
		$(| [
			$({$($pre:tt)*})*
		])?
	) => { /* terminal recursion case */ };
	(
		$target:path : [
			{$($next:tt)*}
			// Remaining invocations
			$($rest:tt)*
		] $(| [
			// Accumulated arguments
			$({$($pre:tt)*})*
		])?
	) => {
		$target!(
			$($($($pre)*,)*)?
			$($next)*
		);
		$crate::lang::tuple::impl_tuples!(
			$target : [
				$($rest)*
			] | [
				$($({$($pre)*})*)?
				{$($next)*}
			]
		);
	};

	// Public
	($target:path; no_unit) => {
		$crate::lang::tuple::impl_tuples!(
			$target : [
				{A: 0}
				{B: 1}
				{C: 2}
				{D: 3}
				{E: 4}
				{F: 5}
				{G: 6}
				{H: 7}
				{I: 8}
				{J: 9}
				{K: 10}
				{L: 11}
			]
		);
	};
	($target:path) => {
		$target!();
		$crate::lang::tuple::impl_tuples!($target; no_unit);
	};
}

use derive_where::derive_where;
// Technically, this re-exports from the macro-prelude, not the local scope. Neat!
pub use impl_tuples;

// === ToOwnedTuple === //

pub trait ToOwnedTuple {
	type Owned;

	fn to_owned(self) -> Self::Owned;

	fn to_owned_by_ref(&self) -> Self::Owned;

	fn as_ref(&self) -> RefToOwnable<'_, Self> {
		RefToOwnable(self)
	}
}

pub trait ToOwnedTupleEq: hash::Hash + Eq + ToOwnedTuple<Owned = Self::_OwnedEq> {
	type _OwnedEq: hash::Hash + Eq;

	fn is_eq_owned(&self, owned: &Self::Owned) -> bool;
}

impl<'a, T: ?Sized + ToOwned> ToOwnedTuple for &'a T {
	type Owned = T::Owned;

	fn to_owned(self) -> Self::Owned {
		ToOwned::to_owned(self)
	}

	fn to_owned_by_ref(&self) -> Self::Owned {
		ToOwned::to_owned(*self)
	}
}

impl<'a, T> ToOwnedTupleEq for &'a T
where
	T: ?Sized + ToOwned + hash::Hash + Eq,
	T::Owned: hash::Hash + Eq,
{
	type _OwnedEq = Self::Owned;

	fn is_eq_owned(&self, owned: &Self::Owned) -> bool {
		*self == owned.borrow()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct PreOwned<T>(pub T);

impl<T: Clone> ToOwnedTuple for PreOwned<T> {
	type Owned = T;

	fn to_owned(self) -> Self::Owned {
		self.0
	}

	fn to_owned_by_ref(&self) -> Self::Owned {
		self.0.clone()
	}
}

impl<T: Clone + Eq + hash::Hash> ToOwnedTupleEq for PreOwned<T> {
	type _OwnedEq = Self::Owned;

	fn is_eq_owned(&self, owned: &Self::Owned) -> bool {
		&self.0 == owned
	}
}

#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[derive_where(Copy, Clone)]
pub struct RefToOwnable<'a, T: ?Sized>(pub &'a T);

impl<T: ?Sized + ToOwnedTuple> ToOwnedTuple for RefToOwnable<'_, T> {
	type Owned = T::Owned;

	fn to_owned(self) -> Self::Owned {
		T::to_owned_by_ref(self.0)
	}

	fn to_owned_by_ref(&self) -> Self::Owned {
		(*self).to_owned()
	}
}

impl<T: ?Sized + ToOwnedTupleEq> ToOwnedTupleEq for RefToOwnable<'_, T> {
	type _OwnedEq = Self::Owned;

	fn is_eq_owned(&self, owned: &Self::Owned) -> bool {
		self.0.is_eq_owned(owned)
	}
}

macro_rules! impl_to_owned_tuple {
	($($name:ident:$field:tt),*) => {
		impl<$($name: ToOwnedTuple),*> ToOwnedTuple for ($($name,)*) {
			type Owned = ($($name::Owned,)*);

			fn to_owned(self) -> Self::Owned {
				($(self.$field.to_owned(),)*)
			}

			fn to_owned_by_ref(&self) -> Self::Owned {
				($(self.$field.to_owned_by_ref(),)*)
			}
		}

		impl<$($name: ToOwnedTupleEq),*> ToOwnedTupleEq for ($($name,)*) {
			type _OwnedEq = Self::Owned;

			#[allow(unused)]
			fn is_eq_owned(&self, owned: &Self::Owned) -> bool {
				$(self.$field.is_eq_owned(&owned.$field) && )* true
			}
		}
	};
}

impl_tuples!(impl_to_owned_tuple);
