use std::{
	any::{type_name, TypeId},
	ptr::{addr_of, addr_of_mut},
};

use crate::lang::{macros::impl_tuples, marker::PhantomInvariant};

// === Provider === //

pub unsafe trait Provider: Sized {
	fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U>;

	fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U>;

	fn try_get<'a, U: ?Sized + 'static>(&'a self) -> Option<&'a U> {
		Self::try_get_raw::<'a, U>(self).map(|p| unsafe { &**p })
	}

	fn try_get_mut<'a, U: ?Sized + 'static>(&'a mut self) -> Option<&'a mut U> {
		Self::try_get_raw_mut::<'a, U>(self).map(|p| unsafe { &mut **p })
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
}

unsafe impl<T: ?Sized + 'static> Provider for &T {
	fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a U>())
		} else {
			None
		}
	}

	fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
		None
	}
}

unsafe impl<T: ?Sized + 'static> Provider for &mut T {
	fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a U>())
		} else {
			None
		}
	}

	fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			Some(me.cast::<&'a mut U>())
		} else {
			None
		}
	}
}

macro tup_impl_provider($($para:ident:$field:tt),*) {
	unsafe impl<$($para: Provider),*> Provider for ($($para,)*) {
		fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
			$(if let Some(p) = <$para as Provider>::try_get_raw(unsafe { addr_of!((*me).$field) }) {
					return Some(p);
			})*

			None
		}

		fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
			$(if let Some(p) = <$para as Provider>::try_get_raw_mut(unsafe { addr_of_mut!((*me).$field) }) {
				return Some(p);
			})*

			None
		}
	}
}

impl_tuples!(tup_impl_provider);

pub struct Exclude<P, E> {
	_excluded_set: PhantomInvariant<E>,
	provider: *mut P,
}

unsafe impl<P: Provider, E> Provider for Exclude<P, E> {
	fn try_get_raw<'a, U: ?Sized + 'static>(me: *const Self) -> Option<*const &'a U> {
		P::try_get_raw(unsafe { (*me).provider })
	}

	fn try_get_raw_mut<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<*mut &'a mut U> {
		P::try_get_raw_mut(unsafe { (*me).provider })
	}
}
