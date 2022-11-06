use std::{
	any::{type_name, TypeId},
	marker::PhantomData,
	ptr::{addr_of, addr_of_mut},
};

use crate::{lang::macros::impl_tuples, mem::alias_magic::has_aliases};

// === Provider === //

pub unsafe trait Provider: Sized {
	unsafe fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U>;

	unsafe fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U>;

	unsafe fn get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> *const &'a U {
		Self::try_get_raw::<U>(me).unwrap_or_else(|| {
			panic!(
				"provider does not have immutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	unsafe fn get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> *mut &'a mut U {
		Self::try_get_raw_mut::<U>(me).unwrap_or_else(|| {
			panic!(
				"provider does not have immutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	fn try_get<'a, U: ?Sized + 'static>(&'a self) -> Option<&'a U> {
		unsafe { Self::try_get_raw::<'a, U>(self).map(|p| &**p) }
	}

	fn try_get_mut<'a, U: ?Sized + 'static>(&'a mut self) -> Option<&'a mut U> {
		unsafe { Self::try_get_raw_mut::<'a, U>(self).map(|p| &mut **p) }
	}

	fn get<U: ?Sized + 'static>(&self) -> &U {
		self.try_get::<U>().unwrap_or_else(|| {
			panic!(
				"provider does not have immutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	fn get_mut<U: ?Sized + 'static>(&mut self) -> &mut U {
		self.try_get_mut::<U>().unwrap_or_else(|| {
			panic!(
				"provider does not have mutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	fn pack<'a, P: ProviderPack<'a>>(&'a mut self) -> P {
		P::pack_from(self)
	}
}

unsafe impl<T: ?Sized + 'static> Provider for &T {
	unsafe fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a U>())
		} else {
			None
		}
	}

	unsafe fn try_get_raw_mut<'a, U: ?Sized + 'static>(_me: *mut Self) -> Option<*mut &'a mut U> {
		None
	}
}

unsafe impl<T: ?Sized + 'static> Provider for &mut T {
	unsafe fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a U>())
		} else {
			None
		}
	}

	unsafe fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a mut U>())
		} else {
			None
		}
	}
}

macro tup_impl_provider($($para:ident:$field:tt),*) {
	unsafe impl<$($para: Provider),*> Provider for ($($para,)*) {
		#[allow(unused)]
		unsafe fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
			$(if let Some(p) = <$para as Provider>::try_get_raw(addr_of!((*me).$field)) {
					return Some(p);
			})*

			None
		}

		#[allow(unused)]
		unsafe fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
			$(if let Some(p) = <$para as Provider>::try_get_raw_mut(addr_of_mut!((*me).$field)) {
				return Some(p);
			})*

			None
		}
	}
}

impl_tuples!(tup_impl_provider);

// === ProviderPack === //

pub struct SpreadProviderPointee {
	_private: (),
}

pub trait ProviderPack<'a> {
	fn pack_from<Q: Provider>(provider: &'a mut Q) -> Self;
}

pub trait ProviderPackPart<'a, P> {
	type AliasPointee: ?Sized + 'static;

	unsafe fn pack_from<Q: Provider>(provider: *mut Q) -> Self;
}

impl<'a, 'p, P, T> ProviderPackPart<'a, P> for &'p T
where
	'a: 'p,
	T: ?Sized + 'static,
{
	type AliasPointee = T;

	unsafe fn pack_from<Q: Provider>(provider: *mut Q) -> Self {
		*Q::get_raw(provider)
	}
}

impl<'a, 'p, P, T> ProviderPackPart<'a, P> for &'p mut T
where
	'a: 'p,
	T: ?Sized + 'static,
{
	type AliasPointee = T;

	unsafe fn pack_from<Q: Provider>(provider: *mut Q) -> Self {
		*Q::get_raw_mut(provider)
	}
}

macro tup_impl_pack($($para:ident:$field:tt),*) {
	impl<'a, $($para: ProviderPackPart<'a, Self>),*> ProviderPack<'a> for ($($para,)*) {
		#[allow(unused)]
		fn pack_from<Q: Provider>(provider: &'a mut Q) -> Self {
			// Check aliasing
			if let Some((offending)) = has_aliases::<(
				$(PhantomData<<$para as ProviderPackPart<'a, Self>>::AliasPointee>,)*
			)>() {
				panic!("{offending:?} was repeated in the pack target. This is not allowed for aliasing reasons.");
			}

			// Pack the tuple
			let provider = provider as *mut Q;

			($( unsafe { <$para as ProviderPackPart<'a, Self>>::pack_from(provider) }, )*)
		}
	}
}

impl_tuples!(tup_impl_pack);

// === Tests === //

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test() {
		let a = 1i32;
		let mut b = 2u32;
		let c = "foo";
		let mut d = (&a, &mut b, c);

		assert_eq!(*d.get::<i32>(), 1);
		receiver(d.pack());
	}

	fn receiver((b, c): (&mut u32, &str)) {
		assert_eq!(*b, 2);
		assert_eq!(c, "foo");
	}
}
