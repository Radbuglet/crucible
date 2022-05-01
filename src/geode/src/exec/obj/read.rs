use crate::exec::atomic_ref_cell::{AMut, ARef, ARefCell, LockError};
use crate::exec::key::{typed_key, RawTypedKey, TypedKey};
use crate::util::arity_utils::{impl_tuples, InjectableClosure};
use crate::util::error::ResultExt;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ptr::NonNull;
use thiserror::Error;

// === Flavor definitions === //

pub trait ObjFlavor: Sized {
	#[doc(hidden)]
	unsafe fn try_acquire_rw_ref<T: ?Sized>(rw: &ARefCell<T>) -> Result<ARef<T>, LockError>;

	#[doc(hidden)]
	unsafe fn try_acquire_rw_mut<T: ?Sized>(rw: &ARefCell<T>) -> Result<AMut<T>, LockError>;
}

pub unsafe trait ObjFlavorCanOwn<T: ?Sized>: ObjFlavor {}

pub struct SendSyncFlavor {
	// Behaves like POD (i.e. is `Send` + `Sync`)
	_private: (),
}

impl ObjFlavor for SendSyncFlavor {
	unsafe fn try_acquire_rw_ref<T: ?Sized>(rw: &ARefCell<T>) -> Result<ARef<T>, LockError> {
		rw.try_borrow()
	}

	unsafe fn try_acquire_rw_mut<T: ?Sized>(rw: &ARefCell<T>) -> Result<AMut<T>, LockError> {
		rw.try_borrow_mut()
	}
}

unsafe impl<T: ?Sized + Send + Sync> ObjFlavorCanOwn<T> for SendSyncFlavor {}

pub struct SendFlavor {
	// Behaves like an `UnsafeCell` (i.e. is `Send` but not `Sync`)
	_private: PhantomData<UnsafeCell<()>>,
}

impl ObjFlavor for SendFlavor {
	unsafe fn try_acquire_rw_ref<T: ?Sized>(rw: &ARefCell<T>) -> Result<ARef<T>, LockError> {
		// Safety: because `Obj` is not `Sync` user this flavor, only one thread can borrow values
		// from it at a given time, allowing us to skip synchronization.
		rw.try_borrow_unsynchronized()
	}

	unsafe fn try_acquire_rw_mut<T: ?Sized>(rw: &ARefCell<T>) -> Result<AMut<T>, LockError> {
		// Safety: because `Obj` is not `Sync` user this flavor, only one thread can borrow values
		// from it at a given time, allowing us to skip synchronization.
		rw.try_borrow_unsynchronized_mut()
	}
}

unsafe impl<T: ?Sized + Send> ObjFlavorCanOwn<T> for SendFlavor {}

pub struct SingleThreadedFlavor {
	// Behaves like a raw pointer (i.e. is neither `Send` nor `Sync`)
	_private: PhantomData<*const ()>,
}

impl ObjFlavor for SingleThreadedFlavor {
	unsafe fn try_acquire_rw_ref<T: ?Sized>(rw: &ARefCell<T>) -> Result<ARef<T>, LockError> {
		// Safety: because `Obj` is not `Sync` user this flavor, only one thread can borrow values
		// from it at a given time, allowing us to skip synchronization.
		rw.try_borrow_unsynchronized()
	}

	unsafe fn try_acquire_rw_mut<T: ?Sized>(rw: &ARefCell<T>) -> Result<AMut<T>, LockError> {
		// Safety: because `Obj` is not `Sync` user this flavor, only one thread can borrow values
		// from it at a given time, allowing us to skip synchronization.
		rw.try_borrow_unsynchronized_mut()
	}
}

unsafe impl<T: ?Sized> ObjFlavorCanOwn<T> for SingleThreadedFlavor {}

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

// === ObjLike === //

pub unsafe trait ObjRead {
	type AccessFlavor: ObjFlavor;

	// === Basic getters === //

	fn try_get_raw<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<NonNull<T>, ComponentMissingError>;

	fn get_raw<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> NonNull<T> {
		self.try_get_raw(key).unwrap_pretty()
	}

	fn try_get_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<&T, ComponentMissingError> {
		self.try_get_raw(key).map(|value| unsafe { value.as_ref() })
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
		Ok(
			unsafe { Self::AccessFlavor::try_acquire_rw_ref(self.try_get_as(key)?) }.map_err(
				|error| ComponentLockError {
					key: key.raw(),
					error,
				},
			)?,
		)
	}

	fn borrow_as<T: ?Sized + 'static>(&self, key: TypedKey<ARefCell<T>>) -> ARef<T> {
		self.try_borrow_as(key).unwrap_pretty()
	}

	fn try_borrow_mut_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<ARefCell<T>>,
	) -> Result<AMut<T>, BorrowError> {
		Ok(
			unsafe { Self::AccessFlavor::try_acquire_rw_mut(self.try_get_as(key)?) }.map_err(
				|error| ComponentLockError {
					key: key.raw(),
					error,
				},
			)?,
		)
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

pub trait MultiBorrowTarget<'a>: Sized {
	fn try_borrow_from<O: ?Sized + ObjRead>(obj: &'a O) -> Result<Self, BorrowError>;
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for &'a T {
	fn try_borrow_from<O: ?Sized + ObjRead>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_get().map_err(From::from)
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for ARef<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjRead>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow()
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for AMut<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjRead>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow_mut()
	}
}

macro impl_tup_obj_borrowable($($name:ident: $field:tt),*) {
impl<'a, $($name: MultiBorrowTarget<'a>),*> MultiBorrowTarget<'a> for ($($name,)*) {
		#[allow(unused_variables)]
		fn try_borrow_from<O: ?Sized + ObjRead>(obj: &'a O) -> Result<Self, BorrowError> {
			Ok(($($name::try_borrow_from(obj)?,)*))
		}
	}
}

impl_tuples!(impl_tup_obj_borrowable);
