use std::{
	marker::Unsize,
	mem,
	ptr::{self, Pointee},
};

pub fn unsize_meta<T, U>(meta: <T as Pointee>::Metadata) -> <U as Pointee>::Metadata
where
	T: ?Sized + Unsize<U>,
	U: ?Sized,
{
	let ptr = ptr::from_raw_parts::<T>(ptr::null(), meta) as *const U;
	let (_, meta) = ptr.to_raw_parts();
	meta
}

pub const unsafe fn dangerous_transmute<A, B>(a: A) -> B {
	if mem::align_of::<A>() != mem::align_of::<B>() {
		panic!("incompatible alignments");
	}

	if mem::size_of::<A>() != mem::size_of::<B>() {
		panic!("incompatible sizes");
	}

	union Punned<A, B> {
		a: mem::ManuallyDrop<A>,
		b: mem::ManuallyDrop<B>,
	}

	let punned = Punned {
		a: mem::ManuallyDrop::new(a),
	};
	mem::ManuallyDrop::into_inner(punned.b)
}
