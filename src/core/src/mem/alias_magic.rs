use std::marker::PhantomData;

use crate::{debug::type_id::NamedTypeId, lang::macros::impl_tuples};

pub unsafe trait NoAliasValidator {
	fn inner_alias() -> Option<NamedTypeId>;
	fn inner_or_specified_alias<T: ?Sized + 'static>() -> Option<NamedTypeId>;
}

unsafe impl NoAliasValidator for () {
	fn inner_alias() -> Option<NamedTypeId> {
		None
	}

	fn inner_or_specified_alias<T: ?Sized + 'static>() -> Option<NamedTypeId> {
		None
	}
}

macro impl_tup($me:ident:$me_field:tt$(,)? $($left:ident:$left_field:tt),*) {
	unsafe impl<$($left: ?Sized + 'static,)* $me: ?Sized + 'static> NoAliasValidator for ($(PhantomData<$left>,)* PhantomData<$me>,) {
		fn inner_alias() -> Option<NamedTypeId> {
			// Check for aliases between me and what is left of me
			<($(PhantomData<$left>,)*)>::inner_or_specified_alias::<$me>()
		}

		fn inner_or_specified_alias<T: ?Sized + 'static>() -> Option<NamedTypeId> {
			// Check for aliases between me and the specified target
			if NamedTypeId::of::<$me>() == NamedTypeId::of::<T>() {
				return Some(NamedTypeId::of::<$me>());
			}

			// Check for aliases between the specified target and what is left of me
			if let tup @ Some(_) = <($(PhantomData<$left>,)*)>::inner_or_specified_alias::<T>() {
				return tup;
			}

			// Check for internal aliases
			if let tup @ Some(_) = Self::inner_alias() {
				return tup;
			}

			None
		}
	}
}

impl_tuples!(impl_tup; no_unit);

pub fn has_aliases<T: NoAliasValidator>() -> Option<NamedTypeId> {
	T::inner_alias()
}
