use crate::transmute::super_unchecked_transmute;
use core::mem::MaybeUninit;

pub const fn new_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	let arr = MaybeUninit::<[T; N]>::uninit();
	transmute_uninit_array_to_inner(arr)
}

#[doc(hidden)]
pub const fn new_uninit_array_and_return_len<T, const N: usize>() -> ([MaybeUninit<T>; N], usize) {
	(new_uninit_array(), N)
}

pub const fn transmute_uninit_array_to_inner<T, const N: usize>(
	arr: MaybeUninit<[T; N]>,
) -> [MaybeUninit<T>; N] {
	unsafe { super_unchecked_transmute(arr) }
}

pub const fn transmute_uninit_array_to_outer<T, const N: usize>(
	arr: [MaybeUninit<T>; N],
) -> MaybeUninit<[T; N]> {
	unsafe { super_unchecked_transmute(arr) }
}

pub fn array_from_iter<I: IntoIterator, const N: usize>(producer: I) -> [I::Item; N] {
	let mut producer = producer.into_iter();
	arr![producer.next().expect("not enough elements in iterator to construct array"); N]
}

pub const fn array_from_copy<T: Copy, const N: usize>(value: T) -> [T; N] {
	arr![value; N]
}

pub macro arr($ctor:expr; $size:expr) {{
	// Construct array
	let (mut arr, len) = new_uninit_array_and_return_len::<_, { $size }>();

	let mut i = 0;

	while i < len {
		arr[i] = MaybeUninit::new($ctor);
		i += 1;
	}

	let arr = transmute_uninit_array_to_outer(arr);
	unsafe { arr.assume_init() }
}}
