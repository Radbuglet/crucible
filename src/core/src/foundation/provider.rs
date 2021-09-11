// TODO: Support nested providers
// TODO: Support deferred init

use crate::util::tuple::impl_tuples;
use std::any::{type_name, TypeId};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

// === Core provider mechanisms === //

pub trait Provider {
	fn provide_raw<'comp>(&'comp self, out: CompOut<'_, 'comp>);
}

#[repr(C)]
pub struct CompOutTarget<'comp, T: ?Sized> {
	head: CompOutTargetHeader,
	comp_ref: MaybeUninit<&'comp T>,
}

#[derive(Clone)]
struct CompOutTargetHeader {
	id: TypeId,
	is_written: bool,
}

impl<'comp, T: ?Sized + 'static> CompOutTarget<'comp, T> {
	pub fn new() -> Self {
		Self {
			head: CompOutTargetHeader {
				id: TypeId::of::<T>(),
				is_written: false,
			},
			comp_ref: MaybeUninit::uninit(),
		}
	}

	pub fn is_set(&self) -> bool {
		self.head.is_written
	}

	pub fn set(&mut self, value: &'comp T) {
		self.comp_ref.write(value);
		self.head.is_written = true;
	}

	pub fn unset(&mut self) {
		self.head.is_written = false;
	}

	pub fn get(self) -> Option<&'comp T> {
		if self.head.is_written {
			Some(unsafe { self.comp_ref.assume_init() })
		} else {
			None
		}
	}

	pub fn writer<'target>(&'target mut self) -> CompOut<'target, 'comp> {
		CompOut::new(self)
	}
}

impl<T: ?Sized + 'static> Clone for CompOutTarget<'_, T> {
	fn clone(&self) -> Self {
		Self {
			head: self.head.clone(),
			comp_ref: self.comp_ref.clone(),
		}
	}
}

pub struct CompOut<'target, 'comp> {
	_ty: PhantomData<&'target mut (bool, &'comp ())>,
	target: NonNull<CompOutTargetHeader>,
}

impl<'target, 'comp> CompOut<'target, 'comp> {
	pub fn new<T: ?Sized>(target: &'target mut CompOutTarget<'comp, T>) -> Self {
		Self {
			_ty: PhantomData,
			target: NonNull::from(target).cast::<CompOutTargetHeader>(),
		}
	}

	fn head(&self) -> &'target CompOutTargetHeader {
		unsafe { self.target.as_ref() }
	}

	pub fn type_id(&self) -> TypeId {
		self.head().id
	}

	pub fn is_written(&self) -> bool {
		self.head().is_written
	}

	pub fn target<T: ?Sized + 'static>(&self) -> Option<&'target CompOutTarget<T>> {
		if TypeId::of::<T>() == self.head().id {
			Some(unsafe { self.target.cast::<CompOutTarget<T>>().as_ref() })
		} else {
			None
		}
	}

	pub fn target_mut<T: ?Sized + 'static>(
		&mut self,
	) -> Option<&'target mut CompOutTarget<'comp, T>> {
		if TypeId::of::<T>() == self.head().id {
			Some(unsafe { self.target.cast::<CompOutTarget<T>>().as_mut() })
		} else {
			None
		}
	}

	pub fn try_provide<T: ?Sized + 'static>(&mut self, comp: &'comp T) -> bool {
		if self.is_written() {
			return true;
		}

		if let Some(target) = self.target_mut::<T>() {
			target.set(comp);
			true
		} else {
			false
		}
	}

	pub fn try_write<T: ?Sized + 'static>(&mut self, comp: &'comp T) -> bool {
		if let Some(target) = self.target_mut::<T>() {
			target.set(comp);
			true
		} else {
			false
		}
	}
}

// === Extension API === //

pub trait ProviderExt {
	fn try_get<T: ?Sized + 'static>(&self) -> Option<&T>;
	fn get<T: ?Sized + 'static>(&self) -> &T;
	fn has<T: ?Sized + 'static>(&self) -> bool;
	fn try_get_many<'a, T: ProviderGetter<'a>>(&'a self) -> Option<T>;
	fn get_many<'a, T: ProviderGetter<'a>>(&'a self) -> T;
}

impl<Target: ?Sized + Provider> ProviderExt for Target {
	fn try_get<T: ?Sized + 'static>(&self) -> Option<&T> {
		let mut out = CompOutTarget::<T>::new();
		self.provide_raw(out.writer());
		out.get()
	}

	fn get<T: ?Sized + 'static>(&self) -> &T {
		let comp = self.try_get::<T>();
		if let Some(comp) = comp {
			comp
		} else {
			panic!("Missing component of type {}!", type_name::<T>());
		}
	}

	fn has<T: ?Sized + 'static>(&self) -> bool {
		self.try_get::<T>().is_some()
	}

	fn try_get_many<'a, T: ProviderGetter<'a>>(&'a self) -> Option<T> {
		T::try_get(self)
	}

	fn get_many<'a, T: ProviderGetter<'a>>(&'a self) -> T {
		T::get(self)
	}
}

pub trait ProviderGetter<'obj>: Sized {
	fn try_get<T: ?Sized + Provider>(obj: &'obj T) -> Option<Self>;

	// "get" is not derived from "try_get" in the "ProviderExt" wrapper because the getter implementation
	// can provide better diagnostics.
	fn get<T: ?Sized + Provider>(obj: &'obj T) -> Self;
}

// === Tuple derivation === //

macro impl_tup($($ty:ident:$field:tt),*) {
	#[allow(unused)]  // in case "out" goes unused
	impl<$($ty: Sized + 'static),*> Provider for ($($ty,)*) {
		fn provide_raw<'comp>(&'comp self, mut out: CompOut<'_, 'comp>) {
			let _ = $(out.try_provide::<$ty>(&self.$field) ||)* false;
		}
	}

	#[allow(unused)]  // in case "out" goes unused
	impl<'obj, $($ty: ?Sized + 'static),*> ProviderGetter<'obj> for ($(&'obj $ty,)*) {
		fn try_get<T: ?Sized + Provider>(obj: &'obj T) -> Option<Self> {
			Some(($(obj.try_get::<$ty>()?,)*))
		}

		fn get<T: ?Sized + Provider>(obj: &'obj T) -> Self {
			($(obj.get::<$ty>(),)*)
		}
	}
}

impl_tuples!(impl_tup);
