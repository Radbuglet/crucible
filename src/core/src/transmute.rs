use core::mem::ManuallyDrop;
use std::mem::MaybeUninit;

pub unsafe trait TransmuteTo<T>: Sized {}

unsafe impl<T> TransmuteTo<T> for T {}

unsafe impl<T, const N: usize> TransmuteTo<[MaybeUninit<T>; N]> for MaybeUninit<[T; N]> {}

unsafe impl<T, const N: usize> TransmuteTo<MaybeUninit<[T; N]>> for [MaybeUninit<T>; N] {}

pub const unsafe fn super_unchecked_transmute<A, B>(a: A) -> B {
	union Punny<A, B> {
		a: ManuallyDrop<A>,
		b: ManuallyDrop<B>,
	}

	let punned = Punny {
		a: ManuallyDrop::new(a),
	};

	ManuallyDrop::into_inner(punned.b)
}

pub fn checked_transmute<A: TransmuteTo<B>, B>(a: A) -> B {
	unsafe {
		// Safety: provided by trait
		super_unchecked_transmute(a)
	}
}
