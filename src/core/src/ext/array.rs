use crate::ext::transmute::entirely_unchecked_transmute;
use crate::transmute::sizealign_checked_transmute;
use core::iter;
use core::mem::MaybeUninit;

// === Array transmute === //

pub const fn transmute_uninit_array_to_inner<T, const N: usize>(
	arr: MaybeUninit<[T; N]>,
) -> [MaybeUninit<T>; N] {
	unsafe { sizealign_checked_transmute(arr) }
}

pub const fn transmute_uninit_array_to_outer<T, const N: usize>(
	arr: [MaybeUninit<T>; N],
) -> MaybeUninit<[T; N]> {
	unsafe { sizealign_checked_transmute(arr) }
}

pub const fn new_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	let arr = MaybeUninit::<[T; N]>::uninit();
	transmute_uninit_array_to_inner(arr)
}

pub const unsafe fn assume_init_array<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> [T; N] {
	transmute_uninit_array_to_outer(arr)
		// Safety: provided by caller
		.assume_init()
}

// === Array constructors === //

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

	pub const unsafe fn unwrap(self) -> [T; N] {
		// Safety: `array` is the first element of this `repr(C)` structure so we can transmute an
		// owned instance of the structure into an owned instance of this field. We also perform an
		// implicit transmute from `[MaybeUninit<T>; N]` to `[T; N]`, whose safety is guaranteed by
		// the caller.
		entirely_unchecked_transmute(self)
	}
}

impl<T, const N: usize> Drop for MacroArrayBuilder<T, N> {
	fn drop(&mut self) {
		for i in 0..self.init_count {
			unsafe {
				// Safety: provided during call to `MacroArrayBuilder::new`.
				self.array[i].assume_init_drop()
			};
		}
	}
}

pub macro arr($ctor:expr; $size:expr) {{
	// N.B. const expressions do not inherit the `unsafe` scope from their surroundings.
	let mut arr = unsafe { MacroArrayBuilder::<_, { $size }>::new() };

	while arr.init_count < arr.len {
		arr.array[arr.init_count] = MaybeUninit::new($ctor);
		arr.init_count += 1;
	}

	unsafe { arr.unwrap() }
}}

pub macro arr_indexed($index:ident => $ctor:expr; $size:expr) {{
	// N.B. const expressions do not inherit the `unsafe` scope from their surroundings.
	let mut arr = unsafe { MacroArrayBuilder::<_, { $size }>::new() };

	while arr.init_count < arr.len {
		arr.array[arr.init_count] = MaybeUninit::new({
			let $index = arr.init_count;
			$ctor
		});
		arr.init_count += 1;
	}

	unsafe { arr.unwrap() }
}}

pub fn arr_from_iter<T, I: IntoIterator<Item = T>, const N: usize>(iter: I) -> [T; N] {
	let mut iter = iter.into_iter();
	let mut count = 0;

	arr![{
		count += 1;
		iter.next().unwrap_or_else(|| panic!("Expected at least {N} element(s); got {}", count - 1))
	}; N]
}

// === Boxed array creation === //

pub fn iter_repeat_len<F, T>(f: F, len: usize) -> iter::Take<iter::RepeatWith<F>>
where
	F: FnMut() -> T,
{
	iter::repeat_with(f).take(len)
}

pub fn vec_from_fn<F, T>(f: F, len: usize) -> Vec<T>
where
	F: FnMut() -> T,
{
	iter_repeat_len(f, len).collect()
}

pub fn boxed_arr_from_fn<F, T>(f: F, len: usize) -> Box<[T]>
where
	F: FnMut() -> T,
{
	vec_from_fn(f, len).into_boxed_slice()
}
