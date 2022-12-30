use std::{
	any,
	cell::{Ref, RefCell, RefMut},
	marker::PhantomData,
	mem,
};

use hashbrown::HashMap;

use crate::{
	debug::{type_id::NamedTypeId, userdata::Userdata},
	lang::macros::impl_tuples,
	mem::inline::MaybeBoxedCopy,
};

// === Core === //

#[derive(Default)]
pub struct Provider<'r> {
	_ty: PhantomData<&'r dyn any::Any>,
	parent: Option<&'r Provider<'r>>,
	values: HashMap<NamedTypeId, (MaybeBoxedCopy<(usize, usize)>, RefCell<()>)>,
}

impl<'r> Provider<'r> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with<T: ProviderEntries<'r>>(entries: T) -> Self {
		Self::new().with(entries)
	}

	pub fn with_parent(parent: Option<&'r Provider<'r>>) -> Self {
		Self {
			_ty: PhantomData,
			parent,
			values: Default::default(),
		}
	}

	pub fn new_with_parent_and_comps<T: ProviderEntries<'r>>(
		parent: Option<&'r Provider<'r>>,
		entries: T,
	) -> Self {
		Self::with_parent(parent).with(entries)
	}

	pub fn parent(&self) -> Option<&'r Provider<'r>> {
		self.parent
	}

	pub fn add_ref<T: ?Sized + 'static>(&mut self, value: &'r T) {
		let sentinel = RefCell::new(());
		mem::forget(sentinel.borrow());

		self.values.insert(
			NamedTypeId::of::<T>(),
			(MaybeBoxedCopy::new(value as *const T), sentinel),
		);
	}

	pub fn add_mut<T: ?Sized + 'static>(&mut self, value: &'r mut T) {
		self.values.insert(
			NamedTypeId::of::<T>(),
			(MaybeBoxedCopy::new(value as *const T), RefCell::new(())),
		);
	}

	fn try_get_entry<T: ?Sized + 'static>(
		&self,
	) -> Option<&(MaybeBoxedCopy<(usize, usize)>, RefCell<()>)> {
		let mut iter = Some(self);

		while let Some(curr) = iter {
			if let Some(entry) = curr.values.get(&NamedTypeId::of::<T>()) {
				return Some(entry);
			}
			iter = curr.parent;
		}

		None
	}

	pub fn try_get<T: ?Sized + 'static>(&self) -> Option<Ref<T>> {
		self.try_get_entry::<T>().map(|(ptr, sentinel)| {
			let guard = sentinel.borrow();

			Ref::map(guard, |_| unsafe {
				let ptr = ptr.get::<*const T>();
				&*ptr
			})
		})
	}

	pub fn get<T: ?Sized + 'static>(&self) -> Ref<T> {
		self.try_get().unwrap_or_else(|| self.comp_not_found::<T>())
	}

	pub fn try_get_mut<T: ?Sized + 'static>(&self) -> Option<RefMut<T>> {
		self.try_get_entry::<T>().map(|(ptr, sentinel)| {
			let guard = sentinel.borrow_mut();

			RefMut::map(guard, |_| unsafe {
				let ptr = ptr.get::<*mut T>();
				&mut *ptr
			})
		})
	}

	pub fn get_mut<T: ?Sized + 'static>(&self) -> RefMut<T> {
		self.try_get_mut()
			.unwrap_or_else(|| self.comp_not_found::<T>())
	}

	fn comp_not_found<T: ?Sized + 'static>(&self) -> ! {
		panic!(
			"Could not find component of type {:?} in provider.\nTypes provided: {:?}",
			NamedTypeId::of::<T>(),
			self.values.keys().copied().collect::<Vec<_>>(),
		);
	}
}

// === Insertion helpers === //

impl<'r> Provider<'r> {
	pub fn with<T: ProviderEntries<'r>>(mut self, item: T) -> Self {
		item.add_to_provider(&mut self);
		self
	}
}

pub trait ProviderEntries<'a> {
	fn add_to_provider(self, provider: &mut Provider<'a>);
	fn add_to_provider_ref(&'a mut self, provider: &mut Provider<'a>);
}

impl<'a: 'b, 'b, T: ?Sized + 'static> ProviderEntries<'b> for &'a T {
	fn add_to_provider(self, provider: &mut Provider<'b>) {
		provider.add_ref(self)
	}

	fn add_to_provider_ref(&'b mut self, provider: &mut Provider<'b>) {
		provider.add_ref(*self)
	}
}

impl<'a: 'b, 'b, T: ?Sized + 'static> ProviderEntries<'b> for &'a mut T {
	fn add_to_provider(self, provider: &mut Provider<'b>) {
		provider.add_mut(self)
	}

	fn add_to_provider_ref(&'b mut self, provider: &mut Provider<'b>) {
		provider.add_mut(*self)
	}
}

macro impl_provider_entries($($para:ident:$field:tt),*) {
	impl<'a, $($para: 'a + ProviderEntries<'a>),*> ProviderEntries<'a> for ($(&'a mut $para,)*) {
		#[allow(unused)]
		fn add_to_provider(self, provider: &mut Provider<'a>) {
			$(self.$field.add_to_provider_ref(&mut *provider);)*
		}

		#[allow(unused)]
		fn add_to_provider_ref(&'a mut self, provider: &mut Provider<'a>) {
			$(self.$field.add_to_provider_ref(&mut *provider);)*
		}
	}
}

impl_tuples!(impl_provider_entries);

// === `unpack!` traits === //

pub trait UnpackTarget<'guard: 'borrow, 'borrow, P: ?Sized> {
	type Guard;
	type Reference;

	fn acquire_guard(src: &'guard P) -> Self::Guard;
	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference;
}

impl<'provider, 'guard: 'borrow, 'borrow, T: ?Sized + Userdata>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for &'borrow T
{
	type Guard = Ref<'guard, T>;
	type Reference = Self;

	fn acquire_guard(src: &'guard Provider) -> Self::Guard {
		src.get()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&*guard
	}
}

impl<'provider, 'guard: 'borrow, 'borrow, T: ?Sized + Userdata>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for &'borrow mut T
{
	type Guard = RefMut<'guard, T>;
	type Reference = Self;

	fn acquire_guard(src: &'guard Provider) -> Self::Guard {
		src.get_mut()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&mut *guard
	}
}

// === `unpack!` macro === //

pub trait UnpackTargetTuple<'guard: 'borrow, 'borrow, P: ?Sized, I> {
	type Output;

	fn acquire_refs(_dummy_provider: &P, input: &'borrow mut I) -> Self::Output;
}

macro impl_guard_tuples_as_refs($($para:ident:$field:tt),*) {
	impl<'guard: 'borrow, 'borrow, P: ?Sized, $($para: UnpackTarget<'guard, 'borrow, P>),*>
		UnpackTargetTuple<'guard, 'borrow, P, ($($para::Guard,)*)>
		for PhantomData<($($para,)*)>
	{
		type Output = ($($para::Reference,)*);

		#[allow(unused)]
		fn acquire_refs(_dummy_provider: &P, guards: &'borrow mut ($($para::Guard,)*)) -> Self::Output {
			($($para::acquire_ref(&mut guards.$field),)*)
		}
	}
}

impl_tuples!(impl_guard_tuples_as_refs);

#[allow(unused_imports)] // (used by the macro)
pub use crate::ecs::universe::{ResourceFromProviderMarker, RwResourceFromProviderMarker};

pub macro unpack {
	// Raw variants
	(__raw $src:expr => $guard:ident & (
		$($ty:ty),*
		$(,)?
	)) => {{
		// Solidify reference
		let src = $src;

		// Acquire guards
		$guard = ($(<$ty as UnpackTarget<_>>::acquire_guard(src),)*);

		// Acquire references
		<PhantomData::<($($ty,)*)> as UnpackTargetTuple<_, _>>::acquire_refs(src, &mut $guard)
	}},
	(__raw $src:expr => (
		$($ty:ty),*
		$(,)?
	)) => {{
		// Solidify reference
		let src = $src;

		// Acquire guards
		($(<$ty as UnpackTarget<'_, '_, _>>::acquire_guard(src),)*)
	}},

	// Type decoding
	(__decode_ty [@res $ty:ty]) => { ResourceFromProviderMarker<&$ty> },
	(__decode_ty [@ref $ty:ty]) => { RwResourceFromProviderMarker<&$ty> },
	(__decode_ty [@mut $ty:ty]) => { RwResourceFromProviderMarker<&mut $ty> },
	(__decode_ty [$ty:ty]) => { $ty },

	// Public interface (w/ guards)
	($src:expr => $guard:ident & (
		$(
			$(@$mode:ident)? $ty:ty
		),*
		$(,)?
	)) => {
		unpack!(__raw $src => $guard & (
			$(unpack!(__decode_ty [ $(@$mode)? $ty ])),*
		))
	},
	($src:expr => $guard:ident & {
		$(
			$name:ident: $(@$mode:ident)? $ty:ty
		),*
		$(,)?
	}) => {
		let ($($name,)*) = unpack!($src => $guard & (
			$($(@$mode)? $ty),*
		));
	},

	// Public interface (w/o guards)
	($src:expr => (
		$(
			$(@$mode:ident)? $ty:ty
		),*
		$(,)?
	)) => {
		unpack!(__raw $src => (
			$(unpack!(__decode_ty [ $(@$mode)? $ty ])),*
		))
	},
	($src:expr => {
		$(
			$name:pat = $(@$mode:ident)? $ty:ty
		),*
		$(,)?
	}) => {
		let ($($name,)*) = unpack!($src => (
			$( $(@$mode)? $ty ),*
		));
	},
}

// === Tuple context passing === //

pub use compost::decompose;
pub use tuples::{CombinConcat, CombinRight};
