use std::{
	any::TypeId,
	marker::PhantomData,
	mem::MaybeUninit,
	ops::{Deref, DerefMut},
	ptr,
};

use crate::{
	macros::{ignore, impl_tuples},
	transmute::sizealign_checked_transmute,
};

// === Pointer Casts === //

pub trait PointeeCastExt {
	type Pointee: ?Sized;

	fn as_byte_ptr(&self) -> *const u8;

	unsafe fn cast_ref_via_ptr<F, R>(&self, f: F) -> &R
	where
		R: ?Sized,
		F: FnOnce(*const Self::Pointee) -> *const R;

	unsafe fn cast_mut_via_ptr<F, R>(&mut self, f: F) -> &mut R
	where
		R: ?Sized,
		F: FnOnce(*mut Self::Pointee) -> *mut R;

	unsafe fn try_cast_ref_via_ptr<F, R, E>(&self, f: F) -> Result<&R, E>
	where
		R: ?Sized,
		F: FnOnce(*const Self::Pointee) -> Result<*const R, E>;

	unsafe fn try_cast_mut_via_ptr<F, R, E>(&mut self, f: F) -> Result<&mut R, E>
	where
		R: ?Sized,
		F: FnOnce(*mut Self::Pointee) -> Result<*mut R, E>;

	unsafe fn transmute_pointee_ref<T: ?Sized>(&self) -> &T;

	unsafe fn transmute_pointee_mut<T: ?Sized>(&mut self) -> &mut T;
}

impl<P: ?Sized> PointeeCastExt for P {
	type Pointee = P;

	fn as_byte_ptr(&self) -> *const u8 {
		self as *const Self as *const u8
	}

	unsafe fn cast_ref_via_ptr<F, R>(&self, f: F) -> &R
	where
		R: ?Sized,
		F: FnOnce(*const Self::Pointee) -> *const R,
	{
		&*f(self)
	}

	unsafe fn cast_mut_via_ptr<F, R>(&mut self, f: F) -> &mut R
	where
		R: ?Sized,
		F: FnOnce(*mut Self::Pointee) -> *mut R,
	{
		&mut *f(self)
	}

	unsafe fn try_cast_ref_via_ptr<F, R, E>(&self, f: F) -> Result<&R, E>
	where
		R: ?Sized,
		F: FnOnce(*const Self::Pointee) -> Result<*const R, E>,
	{
		Ok(&*f(self)?)
	}

	unsafe fn try_cast_mut_via_ptr<F, R, E>(&mut self, f: F) -> Result<&mut R, E>
	where
		R: ?Sized,
		F: FnOnce(*mut Self::Pointee) -> Result<*mut R, E>,
	{
		Ok(&mut *f(self)?)
	}

	unsafe fn transmute_pointee_ref<T: ?Sized>(&self) -> &T {
		sizealign_checked_transmute(self)
	}

	unsafe fn transmute_pointee_mut<T: ?Sized>(&mut self) -> &mut T {
		sizealign_checked_transmute(self)
	}
}

pub fn addr_of_ptr<T: ?Sized>(p: *const T) -> usize {
	p.cast::<()>() as usize
}

// === Runtime type unification === //

pub fn runtime_unify<A: 'static, B: 'static>(a: A) -> B {
	assert_eq!(TypeId::of::<A>(), TypeId::of::<B>());

	unsafe { sizealign_checked_transmute(a) }
}

pub fn runtime_unify_ref<A: ?Sized + 'static, B: ?Sized + 'static>(a: &A) -> &B {
	assert_eq!(TypeId::of::<A>(), TypeId::of::<B>());

	unsafe { sizealign_checked_transmute(a) }
}

pub fn runtime_unify_mut<A: ?Sized + 'static, B: ?Sized + 'static>(a: &mut A) -> &mut B {
	assert_eq!(TypeId::of::<A>(), TypeId::of::<B>());

	unsafe { sizealign_checked_transmute(a) }
}

// === Offset-of === //

pub unsafe trait OffsetOfReprC {
	type OffsetArray: AsRef<[usize]>;

	fn offsets() -> Self::OffsetArray;
}

macro impl_tup_offsets($($para:ident:$field:tt),*) {
	unsafe impl<$($para,)*> OffsetOfReprC for ($($para,)*) {
		type OffsetArray = [usize; 0 $(+ {
			ignore!($para);
			1
		})*];

		#[allow(unused)]  // For empty tuples.
		fn offsets() -> Self::OffsetArray {
			let tup = MaybeUninit::<Self>::uninit();
			let tup_base = tup.as_ptr();

			[$(
				unsafe {
					ptr::addr_of!((*tup_base).$field) as usize - tup_base as usize
				}
			),*]
		}
	}
}

impl_tuples!(impl_tup_offsets);

// === Type Erasure === //

pub trait All {}

impl<T: ?Sized> All for T {}

#[repr(transparent)]
pub struct Incomplete<T> {
	_ty: PhantomData<T>,
	_erased: dyn All,
}

impl<T> Incomplete<T> {
	pub fn new_ref(value: &T) -> &Incomplete<T> {
		unsafe { value.cast_ref_via_ptr(|ptr| ptr as *const dyn All as *const Incomplete<T>) }
	}

	pub fn new_mut(value: &mut T) -> &mut Incomplete<T> {
		unsafe { value.cast_mut_via_ptr(|ptr| ptr as *mut dyn All as *mut Incomplete<T>) }
	}

	pub unsafe fn cast<U>(me: &Self) -> &Incomplete<U> {
		me.cast_ref_via_ptr(|ptr| ptr as *const Incomplete<U>)
	}

	pub unsafe fn cast_mut<U>(me: &mut Self) -> &mut Incomplete<U> {
		me.cast_mut_via_ptr(|ptr| ptr as *mut Incomplete<U>)
	}
}

impl<T> Deref for Incomplete<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.cast_ref_via_ptr(|ptr| ptr as *const T) }
	}
}

impl<T> DerefMut for Incomplete<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { self.cast_mut_via_ptr(|ptr| ptr as *mut T) }
	}
}
