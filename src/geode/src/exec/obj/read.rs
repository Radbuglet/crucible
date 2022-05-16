use crate::exec::atomic_ref_cell::{AMut, ARef, ARefCell, LockError};
use crate::exec::key::{typed_key, RawTypedKey, TypedKey};
use crate::util::arity_utils::{impl_tuples, InjectableClosure};
use crate::util::error::ResultExt;
use std::alloc::Layout;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::{NonNull, Pointee};
use thiserror::Error;

// === RawObj === //

pub trait RawObj: Debug {
	fn provide_raw<'r>(&'r self, out: &mut ProviderOut<'r>);
}

pub struct ProviderTarget<'r, T: ?Sized> {
	_ty: PhantomData<&'r T>,
	inner: ProviderOut<'r>,
	meta: MaybeUninit<<T as Pointee>::Metadata>,
}

impl<'r, T: ?Sized> ProviderTarget<'r, T> {
	pub fn new(key: TypedKey<T>) -> Self {
		Self {
			_ty: PhantomData,
			inner: ProviderOut {
				_ty: PhantomData,
				key: key.raw(),
				base: None,
				p_meta: std::ptr::null_mut(),
				meta_layout: Layout::new::<<T as Pointee>::Metadata>(),
			},
			meta: MaybeUninit::uninit(),
		}
	}

	pub fn handle(&mut self) -> &mut ProviderOut<'r> {
		// This is guaranteed to be pinned for `'_`, hence the creation of a self-referential pointer
		// without the use of `Pin`.
		self.inner.p_meta =
			(&mut self.meta as *mut MaybeUninit<<T as Pointee>::Metadata>).cast::<u8>();

		// Now, we just return a reference to `inner`. We can still prevent us from promoting `p_meta`
		// to `&mut ...`.
		&mut self.inner
	}

	pub fn get(&self) -> Option<&'r T> {
		if let Some(base) = self.inner.base {
			let ptr = unsafe {
				let meta = *self.meta.assume_init_ref();
				NonNull::from_raw_parts(base, meta).as_ref()
			};
			Some(ptr)
		} else {
			None
		}
	}
}

pub struct ProviderOut<'r> {
	_ty: PhantomData<&'r ()>,
	key: RawTypedKey,
	base: Option<NonNull<()>>,
	p_meta: *mut u8,
	meta_layout: Layout,
}

impl<'r> ProviderOut<'r> {
	pub fn meta_layout(&self) -> Layout {
		self.meta_layout
	}

	pub unsafe fn provide_dynamic_unchecked(&mut self, base_ptr: NonNull<()>, p_meta: *const u8) {
		self.base = Some(base_ptr);
		self.p_meta
			.cast::<u8>()
			.copy_from(p_meta, self.meta_layout.size());
	}

	pub unsafe fn provide_unchecked<T: ?Sized>(&mut self, value: &'r T) {
		let (base, meta) = NonNull::from(value).to_raw_parts();
		self.base = Some(base);
		self.p_meta.cast::<<T as Pointee>::Metadata>().write(meta);
	}

	pub fn provide<T: ?Sized>(&mut self, key: TypedKey<T>, ptr: &'r T) -> bool {
		if self.key == key.raw() {
			unsafe { self.provide_unchecked(ptr) };
			true
		} else {
			false
		}
	}

	pub fn key(&self) -> RawTypedKey {
		self.key
	}

	pub fn did_provide(&self) -> bool {
		self.base.is_some()
	}
}

// === Error types === //

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("component {key:?} missing from `Obj`")]
pub struct ComponentMissingError {
	pub key: RawTypedKey,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("failed to lock component with key {key:?}")]
pub struct ComponentLockError {
	pub error: LockError,
	pub key: RawTypedKey,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
pub enum BorrowError {
	#[error("failed to find component in `Obj`")]
	ComponentMissing(#[from] ComponentMissingError),
	#[error("failed to borrow component from `Obj`")]
	LockError(#[from] ComponentLockError),
}

// === ObjExt === //

pub trait ObjExt: RawObj {
	fn try_get_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<&T, ComponentMissingError> {
		let mut target = ProviderTarget::new(key);
		self.provide_raw(target.handle());
		target.get().ok_or(ComponentMissingError { key: key.raw() })
	}

	fn get_as<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> &T {
		self.try_get_as(key).unwrap_pretty()
	}

	fn try_get<T: ?Sized + 'static>(&self) -> Result<&T, ComponentMissingError> {
		self.try_get_as(typed_key::<T>())
	}

	fn get<T: ?Sized + 'static>(&self) -> &T {
		self.try_get().unwrap_pretty()
	}

	// === Borrow getters === //

	fn try_borrow_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<ARefCell<T>>,
	) -> Result<ARef<T>, BorrowError> {
		Ok(self
			.try_get_as(key)?
			.try_borrow()
			.map_err(|error| ComponentLockError {
				key: key.raw(),
				error,
			})?)
	}

	fn borrow_as<T: ?Sized + 'static>(&self, key: TypedKey<ARefCell<T>>) -> ARef<T> {
		self.try_borrow_as(key).unwrap_pretty()
	}

	fn try_borrow_mut_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<ARefCell<T>>,
	) -> Result<AMut<T>, BorrowError> {
		Ok(self
			.try_get_as(key)?
			.try_borrow_mut()
			.map_err(|error| ComponentLockError {
				key: key.raw(),
				error,
			})?)
	}

	fn borrow_mut_as<T: ?Sized + 'static>(&self, key: TypedKey<ARefCell<T>>) -> AMut<T> {
		self.try_borrow_mut_as(key).unwrap_pretty()
	}

	fn try_borrow<T: ?Sized + 'static>(&self) -> Result<ARef<T>, BorrowError> {
		self.try_borrow_as(typed_key::<ARefCell<T>>())
	}

	fn borrow<T: ?Sized + 'static>(&self) -> ARef<T> {
		self.try_borrow::<T>().unwrap_pretty()
	}

	fn try_borrow_mut<T: ?Sized + 'static>(&self) -> Result<AMut<T>, BorrowError> {
		self.try_borrow_mut_as(typed_key::<ARefCell<T>>())
	}

	fn borrow_mut<T: ?Sized + 'static>(&self) -> AMut<T> {
		self.try_borrow_mut::<T>().unwrap_pretty()
	}

	// === Multi-getters === //

	fn try_borrow_many<'a, D: MultiBorrowTarget<'a>>(&'a self) -> Result<D, BorrowError> {
		D::try_borrow_from(self)
	}

	fn borrow_many<'a, D: MultiBorrowTarget<'a>>(&'a self) -> D {
		self.try_borrow_many().unwrap_pretty()
	}

	fn inject<'a, D, F>(&'a self, mut handler: F) -> F::Return
	where
		D: MultiBorrowTarget<'a>,
		F: InjectableClosure<(), D>,
	{
		handler.call_injected((), self.borrow_many())
	}

	fn inject_with<'a, A, D, F>(&'a self, mut handler: F, args: A) -> F::Return
	where
		D: MultiBorrowTarget<'a>,
		F: InjectableClosure<A, D>,
	{
		handler.call_injected(args, self.borrow_many())
	}
}

impl<T: ?Sized + RawObj> ObjExt for T {}

pub trait MultiBorrowTarget<'a>: Sized {
	fn try_borrow_from<O: ?Sized + ObjExt>(obj: &'a O) -> Result<Self, BorrowError>;
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for &'a T {
	fn try_borrow_from<O: ?Sized + ObjExt>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_get().map_err(From::from)
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for ARef<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjExt>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow()
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for AMut<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjExt>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow_mut()
	}
}

macro impl_tup_obj_borrowable($($name:ident: $field:tt),*) {
impl<'a, $($name: MultiBorrowTarget<'a>),*> MultiBorrowTarget<'a> for ($($name,)*) {
		#[allow(unused_variables)]
		fn try_borrow_from<O: ?Sized + ObjExt>(obj: &'a O) -> Result<Self, BorrowError> {
			Ok(($($name::try_borrow_from(obj)?,)*))
		}
	}
}

impl_tuples!(impl_tup_obj_borrowable);
