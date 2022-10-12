use core::mem::{self, ManuallyDrop};

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

pub unsafe fn cast_ref_via_ptr<T, U, F>(val: &T, f: F) -> &U
where
	T: ?Sized,
	U: ?Sized,
	F: FnOnce(*const T) -> *const U,
{
	&*f(val)
}

pub unsafe fn cast_mut_via_ptr<T, U, F>(val: &mut T, f: F) -> &mut U
where
	T: ?Sized,
	U: ?Sized,
	F: FnOnce(*mut T) -> *mut U,
{
	&mut *f(val)
}

pub const unsafe fn prolong_ref<'r, T: ?Sized>(val: &T) -> &'r T {
	&*(val as *const T)
}

pub unsafe fn prolong_ref_mut<'r, T: ?Sized>(val: &mut T) -> &'r mut T {
	&mut *(val as *mut T)
}
