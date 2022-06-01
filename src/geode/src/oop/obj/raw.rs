use crate::oop::atomic_ref_cell::{AMut, ARef, ARefCell, LockError};
use crate::oop::key::{typed_key, RawTypedKey, TypedKey};
use crate::util::arity_utils::{impl_tuples, InjectableClosure};
use crate::util::error::ResultExt;
use std::alloc::Layout;
use std::fmt::Debug;
use std::marker::PhantomData;
use thiserror::Error;

// === RawObj === //

pub trait RawObj: Debug {
	fn provide_raw<'t, 'r>(&'r self, out: &mut ProviderOut<'t, 'r>);
}

pub struct ProviderOut<'t, 'r> {
	key: RawTypedKey,
	_p_target_ty: PhantomData<&'t mut Option<&'r ()>>,
	p_target: *mut u8,
	ptr_layout: Layout,
	did_provide: bool,
}

unsafe impl<'t, 'r> Send for ProviderOut<'t, 'r> {}
unsafe impl<'t, 'r> Sync for ProviderOut<'t, 'r> {}

impl<'t, 'r> ProviderOut<'t, 'r> {
	pub fn new<T: ?Sized>(key: TypedKey<T>, target: &'t mut Option<&'r T>) -> Self {
		let is_set = target.is_some();
		Self {
			key: key.raw(),
			_p_target_ty: PhantomData,
			p_target: target as *mut Option<&'r T> as *mut u8,
			ptr_layout: Layout::new::<&'r T>(),
			did_provide: is_set,
		}
	}

	pub unsafe fn provide_dynamic_unchecked(&mut self, p_ptr: *const u8) {
		// Safety Considerations:
		// - `Option<&'r T>` has no `Drop` implementation so we don't need to remember to call it.
		// - `&'r T` is transmutable to `Option<&'r T>`.
		// - `ptr_layout.size()` is equal to the size of `Option<&'r T>`.
		// - TODO: Ensure that copying pointer-tagged bytes as if they were regular bytes is valid.
		//    Technically, unlike the example of storing pointer bytes in a usize which *is UB*, we're
		//    never violating allocation target layouts—we're storing pointer bytes into a container
		//    of pointer bytes. However, there's no saying what `copy_from_nonoverlapping` might
		//    be doing internally (e.g. loading each byte into a `u8` temporary, which could cause UB).
		//    There have been whisperings in the "Rust Programming Language Community" discord guild
		//    that the former is guaranteed behavior but I'm waiting for clarification from the docs
		//    to remove this to-do.
		std::ptr::copy_nonoverlapping(p_ptr, self.p_target, self.ptr_layout.size());
		self.did_provide = true;
	}

	pub unsafe fn provide_unchecked<T: ?Sized>(&mut self, value: &'r T) {
		let p_ref = self.p_target as *mut Option<&'r T>;
		*p_ref = Some(value);
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

	pub fn ptr_layout(&self) -> Layout {
		self.ptr_layout
	}

	pub fn did_provide(&self) -> bool {
		self.did_provide
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
		let mut target = None;
		self.provide_raw(&mut ProviderOut::new(key, &mut target));
		target.ok_or(ComponentMissingError { key: key.raw() })
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
