use std::{
	any::{type_name, TypeId},
	marker::PhantomData,
	mem::{self, ManuallyDrop},
	ops::{Deref, DerefMut},
};

// === Transmute === //

pub const unsafe fn entirely_unchecked_transmute<A, B>(a: A) -> B {
	union Punny<A, B> {
		a: ManuallyDrop<A>,
		b: ManuallyDrop<B>,
	}

	let punned = Punny {
		a: ManuallyDrop::new(a),
	};

	ManuallyDrop::into_inner(punned.b)
}

pub const unsafe fn sizealign_checked_transmute<A, B>(a: A) -> B {
	assert!(mem::size_of::<A>() == mem::size_of::<B>());
	assert!(mem::align_of::<A>() >= mem::align_of::<B>());

	entirely_unchecked_transmute(a)
}

// === Allocation === //

pub fn leak_on_heap<'a, T>(val: T) -> &'a mut T {
	Box::leak(Box::new(val))
}

// === Pointer Casts === //

pub trait PointeeCastExt {
	type Pointee: ?Sized;

	fn as_byte_ptr(&self) -> *const u8;

	unsafe fn prolong<'r>(&self) -> &'r Self::Pointee;

	unsafe fn prolong_mut<'r>(&mut self) -> &'r mut Self::Pointee;

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

	unsafe fn prolong<'r>(&self) -> &'r Self::Pointee {
		&*(self as *const Self::Pointee)
	}

	unsafe fn prolong_mut<'r>(&mut self) -> &'r mut Self::Pointee {
		&mut *(self as *mut Self::Pointee)
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

pub trait HeapPointerExt {
	type Pointee: ?Sized;

	unsafe fn prolong_heap_ref<'a>(&self) -> &'a Self::Pointee;
}

impl<T: ?Sized> HeapPointerExt for Box<T> {
	type Pointee = T;

	unsafe fn prolong_heap_ref<'a>(&self) -> &'a Self::Pointee {
		(&**self).prolong()
	}
}

pub fn addr_of_ptr<T: ?Sized>(p: *const T) -> usize {
	p.cast::<()>() as usize
}

// === Runtime type unification === //

pub fn are_probably_equal<A: ?Sized, B: ?Sized>() -> bool {
	type_name::<A>() == type_name::<B>()
}

pub fn try_runtime_unify<A: 'static, B: 'static>(a: A) -> Option<B> {
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

pub fn try_runtime_unify_ref<A: ?Sized + 'static, B: ?Sized + 'static>(a: &A) -> Option<&B> {
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

pub fn try_runtime_unify_mut<A, B>(a: &mut A) -> Option<&mut B>
where
	A: ?Sized + 'static,
	B: ?Sized + 'static,
{
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

pub fn runtime_unify<A: 'static, B: 'static>(a: A) -> B {
	try_runtime_unify(a).unwrap()
}

pub fn runtime_unify_ref<A: ?Sized + 'static, B: ?Sized + 'static>(a: &A) -> &B {
	try_runtime_unify_ref(a).unwrap()
}

pub fn runtime_unify_mut<A: ?Sized + 'static, B: ?Sized + 'static>(a: &mut A) -> &mut B {
	try_runtime_unify_mut(a).unwrap()
}

pub unsafe fn unchecked_unify<A, B>(a: A) -> B {
	assert!(are_probably_equal::<A, B>());
	sizealign_checked_transmute(a)
}

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
