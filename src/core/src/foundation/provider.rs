use crate::foundation::{RwLock, RwLockManager};
use crate::util::tuple::impl_tuples;
use once_cell::sync::OnceCell;
use std::any::{type_name, TypeId};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

// === Core provider mechanisms === //

pub trait Provider {
	fn provide_raw<'comp>(&'comp self, out: &mut CompOut<'_, 'comp>);
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
		if let Some(target) = self.target_mut::<T>() {
			target.set(comp);
			true
		} else {
			false
		}
	}
}

// This function is used so that we can ensure that `provider` implements [Provider].
#[doc(hidden)]
pub fn forward_macro_util<'comp, T: 'comp + ?Sized + Provider>(
	out: &mut CompOut<'_, 'comp>,
	provider: &'comp T,
) {
	provider.provide_raw(out)
}

pub macro try_provide($out:expr, $value:expr) {{
	let out: &mut CompOut<'_, '_> = $out;
	if out.try_provide($value) {
		return;
	}
}}

pub macro try_forward($out:expr, $handler:expr) {{
	let out: &mut CompOut<'_, '_> = $out;
	forward_macro_util(out, $handler);
	if $out.is_written() {
		return;
	}
}}

pub macro provider_struct($(
	$(#[$item_attr:meta])*
	$vis:vis struct $struct_name:ident {
		$(
			$(#[$field_attr:meta])*
			$field_name:ident: $field_ty:ty
		),*
		$(,)?
	}
)*) {$(
	$(#[$item_attr])*
	$vis struct $struct_name {
		$(
			$(#[$field_attr])*
			$field_name: $field_ty
		),*
	}

	impl Provider for $struct_name {
		fn provide_raw<'comp>(&'comp self, out: &mut CompOut<'_, 'comp>) {
			$( try_provide!(out, &self.$field_name); )*
		}
	}
)*}

// === Extension API === //

pub trait ProviderExt {
	fn try_get<T: ?Sized + 'static>(&self) -> Option<&T>;
	fn get<T: ?Sized + 'static>(&self) -> &T;
	fn has<T: ?Sized + 'static>(&self) -> bool;
	fn try_get_many<'a, T: ProviderGetter<'a>>(&'a self) -> Option<T>;
	fn get_many<'a, T: ProviderGetter<'a>>(&'a self) -> T;
	fn has_many<'a, T: ProviderGetter<'a>>(&'a self) -> bool;
}

impl<Target: ?Sized + Provider> ProviderExt for Target {
	fn try_get<T: ?Sized + 'static>(&self) -> Option<&T> {
		let mut out = CompOutTarget::<T>::new();
		self.provide_raw(&mut out.writer());
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

	fn has_many<'a, T: ProviderGetter<'a>>(&'a self) -> bool {
		self.try_get_many::<T>().is_some()
	}
}

pub trait ProviderGetter<'obj>: Sized {
	fn try_get<T: ?Sized + Provider>(obj: &'obj T) -> Option<Self>;

	// "get" is not derived from "try_get" in the "ProviderExt" wrapper because the getter implementation
	// can provide better diagnostics.
	fn get<T: ?Sized + Provider>(obj: &'obj T) -> Self;
}

pub macro get_many($target:expr, $($name:ident: $ty:ty),+$(,)?) {
	let ($($name,)*) = ProviderExt::get_many::<($($ty,)*)>($target);
}

// === Standard static providers === //

#[derive(Default)]
pub struct Component<T: ?Sized>(pub T);

impl<T: ?Sized + 'static> Provider for Component<T> {
	fn provide_raw<'comp>(&'comp self, out: &mut CompOut<'_, 'comp>) {
		out.try_provide(&self.0);
	}
}

#[derive(Default)]
pub struct MultiProvider<T>(pub T);

macro impl_tup($($ty:ident:$field:tt),*) {
	#[allow(unused)]  // in case "out" goes unused
	impl<$($ty: Provider),*> Provider for MultiProvider<($($ty,)*)> {
		fn provide_raw<'comp>(&'comp self, out: &mut CompOut<'_, 'comp>) {
			$(try_forward!(out, &self.0.$field);)*
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

// === Lazy components === //

pub trait LazyProviderExt {
	fn try_init<T: 'static>(&self, value: T) -> bool;

	fn init<T: 'static>(&self, value: T) {
		let success = self.try_init(value);
		assert!(
			success,
			"Cannot initialize {}: already initialized.",
			type_name::<T>()
		);
	}

	fn get_or_init<T: 'static, F: FnOnce() -> T>(&self, init: F) -> &T;
}

impl<Target: ?Sized + Provider> LazyProviderExt for Target {
	fn try_init<T: 'static>(&self, value: T) -> bool {
		self.get::<LazyComponent<T>>().try_init(value)
	}

	fn get_or_init<T: 'static, F: FnOnce() -> T>(&self, init: F) -> &T {
		self.get::<LazyComponent<T>>().get_or_init(init)
	}
}

pub struct LazyComponent<T> {
	value: OnceCell<T>,
}

impl<T> Default for LazyComponent<T> {
	fn default() -> Self {
		Self {
			value: OnceCell::new(),
		}
	}
}

impl<T> LazyComponent<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn try_init(&self, value: T) -> bool {
		self.value.try_insert(value).is_ok()
	}

	pub fn init(&self, value: T) {
		let success = self.try_init(value);
		assert!(
			success,
			"Failed to initialize {}: value already initialized",
			type_name::<Self>()
		);
	}

	pub fn try_get(&self) -> Option<&T> {
		self.value.get()
	}

	pub fn get(&self) -> &T {
		self.try_get().unwrap()
	}

	pub fn get_or_init<F>(&self, init: F) -> &T
	where
		F: FnOnce() -> T,
	{
		self.value.get_or_try_init::<_, !>(|| Ok(init())).unwrap()
	}
}

impl<T: 'static> Provider for LazyComponent<T> {
	fn provide_raw<'comp>(&'comp self, out: &mut CompOut<'_, 'comp>) {
		// Write lazy wrapper for init
		try_provide!(out, self);

		// Write inner value if available
		if let Some(inner) = self.value.get() {
			try_provide!(out, inner);
		}
	}
}

// === Integration with RwLocks === //

pub type RwLockComponent<T> = LazyComponent<RwLock<T>>;

pub trait ProviderRwLockExt: Provider {
	fn init_lock<T: 'static>(&self, value: T);
	fn get_lock<T: 'static>(&self) -> &RwLock<T>;
}

impl<Target: ?Sized + Provider> ProviderRwLockExt for Target {
	fn init_lock<T: 'static>(&self, value: T) {
		self.init(RwLock::new(self.get::<RwLockManager>().clone(), value))
	}

	fn get_lock<T: 'static>(&self) -> &RwLock<T> {
		self.get::<RwLock<T>>()
	}
}
