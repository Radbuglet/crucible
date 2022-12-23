use std::{
	any,
	cell::{Ref, RefCell, RefMut},
	marker::PhantomData,
	mem,
};

use hashbrown::HashMap;

use crate::{debug::type_id::NamedTypeId, mem::inline::MaybeBoxedCopy};

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

	pub fn with_parent(parent: Option<&'r Provider<'r>>) -> Self {
		Self {
			_ty: PhantomData,
			parent,
			values: Default::default(),
		}
	}

	pub fn parent(&self) -> Option<&'r Provider<'r>> {
		self.parent
	}

	pub fn with<T: ProviderRef<'r>>(mut self, item: T) -> Self {
		item.add_to_provider(&mut self);
		self
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

pub trait ProviderRef<'a> {
	fn add_to_provider(self, provider: &mut Provider<'a>);
}

impl<'a: 'b, 'b, T: ?Sized + 'static> ProviderRef<'b> for &'a T {
	fn add_to_provider(self, provider: &mut Provider<'b>) {
		provider.add_ref(self)
	}
}

impl<'a: 'b, 'b, T: ?Sized + 'static> ProviderRef<'b> for &'a mut T {
	fn add_to_provider(self, provider: &mut Provider<'b>) {
		provider.add_mut(self)
	}
}

pub trait ProviderPack<'a> {
	fn get_from_provider(provider: &'a Provider) -> Self;
}

impl<'a, T: ?Sized + 'static> ProviderPack<'a> for Ref<'a, T> {
	fn get_from_provider(provider: &'a Provider) -> Self {
		provider.get()
	}
}

impl<'a, T: ?Sized + 'static> ProviderPack<'a> for RefMut<'a, T> {
	fn get_from_provider(provider: &'a Provider) -> Self {
		provider.get_mut()
	}
}

#[allow(unused)] // Unused in macro
use crate::lang::macros::ignore;

pub macro unpack(
	$src:expr => {
		$($name:ident: $ty:ty),*
		$(,)?
	}
)  {
	#[allow(unused_mut)]
	let ($(mut $name,)*): ($($ty,)*) = {
		let src = &$src;

		($({
			ignore!($name);
			ProviderPack::get_from_provider(src)
		},)*)
	};
}
