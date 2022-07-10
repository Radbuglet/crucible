use crate::transmute::super_unchecked_transmute;
use core::mem::MaybeUninit;

pub const fn new_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	let arr = MaybeUninit::<[T; N]>::uninit();
	transmute_uninit_array_to_inner(arr)
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

pub const unsafe fn assume_init_array<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> [T; N] {
	// Safety: provided by caller
	transmute_uninit_array_to_outer(arr).assume_init()
}

#[repr(C)]
pub struct MacroArrayBuilder<T, const N: usize> {
	pub array: [MaybeUninit<T>; N],
	pub init_count: usize,
	pub len: usize,
}

impl<T, const N: usize> MacroArrayBuilder<T, N> {
	pub const unsafe fn new() -> Self {
		Self {
			array: new_uninit_array(),
			init_count: 0,
			len: N,
		}
	}
}

impl<T, const N: usize> Drop for MacroArrayBuilder<T, N> {
	fn drop(&mut self) {
		for i in 0..self.init_count {
			unsafe { self.array[i].assume_init_drop() };
		}
	}
}

pub const unsafe fn unwrap_macro_array_builder<T, const N: usize>(
	builder: MacroArrayBuilder<T, N>,
) -> [T; N] {
	super_unchecked_transmute(builder)
}

pub macro arr($ctor:expr; $size:expr) {{
	let mut arr = unsafe { MacroArrayBuilder::<_, { $size }>::new() };

	while arr.init_count < arr.len {
		arr.array[arr.init_count] = MaybeUninit::new($ctor);
		arr.init_count += 1;
	}

	unsafe { unwrap_macro_array_builder(arr) }
}}
