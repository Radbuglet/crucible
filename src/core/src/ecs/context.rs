use std::{
	any::{type_name, TypeId},
	collections::HashMap,
	fmt,
	marker::PhantomData,
	ptr::{addr_of, addr_of_mut},
};

use crate::{
	debug::type_id::NamedTypeId,
	lang::macros::impl_tuples,
	mem::{alias_magic::has_aliases, inline::MaybeBoxedCopy, ptr::All},
};

// === Provider === //

pub unsafe trait Provider: Sized {
	// === Required methods === //

	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>);

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U>;

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		me: *mut Self,
	) -> Option<&'a mut U>;

	// === Derived getters === //

	unsafe fn get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> &'a U {
		Self::try_get_comp_unchecked::<U>(me).unwrap_or_else(|| {
			panic!(
				"provider does not have immutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	unsafe fn get_comp_mut_unchecked<'a, U: ?Sized + 'static>(me: *mut Self) -> &'a mut U {
		Self::try_get_comp_mut_unchecked::<U>(me).unwrap_or_else(|| {
			panic!(
				"provider does not have mutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	fn try_get_comp<'a, U: ?Sized + 'static>(&'a self) -> Option<&'a U> {
		unsafe { Self::try_get_comp_unchecked::<'a, U>(self) }
	}

	fn try_get_comp_mut<'a, U: ?Sized + 'static>(&'a mut self) -> Option<&'a mut U> {
		unsafe { Self::try_get_comp_mut_unchecked::<'a, U>(self) }
	}

	fn get_comp<U: ?Sized + 'static>(&self) -> &U {
		self.try_get_comp::<U>().unwrap_or_else(|| {
			panic!(
				"provider does not have immutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	fn get_comp_mut<U: ?Sized + 'static>(&mut self) -> &mut U {
		self.try_get_comp_mut::<U>().unwrap_or_else(|| {
			panic!(
				"provider does not have mutable component of type {:?}",
				type_name::<U>()
			)
		})
	}

	// === Conversions === //

	fn pack<'a, P: ProviderPack<'a>>(&'a mut self) -> P {
		P::pack_from(self)
	}

	fn as_dyn(&mut self) -> DynProvider {
		let mut target = DynProvider::default();
		self.build_dyn_provider(&mut target);
		target
	}
}

unsafe impl<T: ?Sized + 'static> Provider for &T {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		provider.add_ref(*self);
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			let p_me = me.cast::<*const U>().read(); // &T -> *const T -> *const U
			Some(&*p_me)
		} else {
			None
		}
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		_me: *mut Self,
	) -> Option<&'a mut U> {
		None
	}
}

unsafe impl<T: ?Sized + 'static> Provider for &mut T {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		provider.add_mut(*self);
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			let p_me = me.cast::<*const U>().read(); // &mut T -> *mut T -> *const T -> *const U
			Some(&*p_me)
		} else {
			None
		}
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		me: *mut Self,
	) -> Option<&'a mut U> {
		if TypeId::of::<T>() == TypeId::of::<U>() {
			let p_me = me.cast::<*mut U>().read(); // &mut T -> *mut T -> *mut U
			Some(&mut *p_me)
		} else {
			None
		}
	}
}

macro tup_impl_provider($($para:ident:$field:tt),*) {
	unsafe impl<$($para: Provider),*> Provider for ($($para,)*) {
		#[allow(unused)]
		fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
			$(self.$field.build_dyn_provider(provider);)*
		}

		#[allow(unused)]
		unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U> {
			$(if let Some(p) = <$para as Provider>::try_get_comp_unchecked(addr_of!((*me).$field)) {
				return Some(p);
			})*

			None
		}

		#[allow(unused)]
		unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(me: *mut Self) -> Option<&'a mut U> {
			$(if let Some(p) = <$para as Provider>::try_get_comp_mut_unchecked(addr_of_mut!((*me).$field)) {
				return Some(p);
			})*

			None
		}
	}
}

impl_tuples!(tup_impl_provider);

// === DynProvider === //

#[derive(Default)]
pub struct DynProvider<'r> {
	_ty: PhantomData<&'r dyn All>,
	comps: HashMap<NamedTypeId, (bool, MaybeBoxedCopy<*mut dyn All>)>,
}

impl fmt::Debug for DynProvider<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DynProvider")
			.field("comps", &self.comps.keys().collect::<Vec<_>>())
			.finish()
	}
}

impl<'r> DynProvider<'r> {
	pub fn add_ref<T: ?Sized + 'static>(&mut self, comp: &'r T) {
		unsafe { self.add_ref_raw(comp) }
	}

	pub fn add_mut<T: ?Sized + 'static>(&mut self, comp: &'r mut T) {
		unsafe { self.add_mut_raw(comp) }
	}

	pub unsafe fn add_raw<T: ?Sized + 'static>(&mut self, is_mutable: bool, comp: *const T) {
		self.comps.insert(
			NamedTypeId::of::<T>(),
			(is_mutable, MaybeBoxedCopy::new(comp)),
		);
	}

	pub unsafe fn add_ref_raw<T: ?Sized + 'static>(&mut self, comp: *const T) {
		self.add_raw(false, comp);
	}

	pub unsafe fn add_mut_raw<T: ?Sized + 'static>(&mut self, comp: *mut T) {
		self.add_raw(true, comp);
	}

	pub fn remove<T: ?Sized + 'static>(&mut self) {
		self.comps.remove(&NamedTypeId::of::<T>());
	}
}

unsafe impl Provider for DynProvider<'_> {
	fn build_dyn_provider<'r>(&'r mut self, provider: &mut DynProvider<'r>) {
		for (&key, (is_mutable, comp)) in &self.comps {
			provider.comps.insert(key, (*is_mutable, comp.clone()));
		}
	}

	unsafe fn try_get_comp_unchecked<'a, U: ?Sized + 'static>(me: *const Self) -> Option<&'a U> {
		let me = &*me;

		me.comps
			.get(&NamedTypeId::of::<U>())
			.map(|(_mutable, ptr)| &*ptr.get::<*const U>())
	}

	unsafe fn try_get_comp_mut_unchecked<'a, U: ?Sized + 'static>(
		me: *mut Self,
	) -> Option<&'a mut U> {
		let me = &*me;

		me.comps
			.get(&NamedTypeId::of::<U>())
			.and_then(|(mutable, ptr)| {
				if *mutable {
					Some(&mut *ptr.get::<*mut U>())
				} else {
					None
				}
			})
	}
}

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
		Q::get_comp_unchecked(provider)
	}
}

impl<'a, 'p, P, T> ProviderPackPart<'a, P> for &'p mut T
where
	'a: 'p,
	T: ?Sized + 'static,
{
	type AliasPointee = T;

	unsafe fn pack_from<Q: Provider>(provider: *mut Q) -> Self {
		Q::get_comp_mut_unchecked(provider)
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

// === `unpack` === //

pub macro unpack(
	$src:expr => {
		$($name:pat_param = $ty:ty),*
		$(,)?
	}
) {
	let ($($name,)*): ($($ty,)*) = Provider::pack($src);
}

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

		assert_eq!(*d.get_comp::<i32>(), 1);
		receiver(d.pack());
	}

	fn receiver(mut cx: (&mut u32, &str)) {
		assert_eq!(*cx.0, 2);
		assert_eq!(cx.1, "foo");

		receiver_2(&mut cx.as_dyn());
	}

	fn receiver_2(cx: &mut DynProvider) {
		dbg!(&cx);
		unpack!(cx => {
			b = &mut u32,
			c = &str,
		});

		assert_eq!(*b, 2);
		assert_eq!(c, "foo");
	}
}
