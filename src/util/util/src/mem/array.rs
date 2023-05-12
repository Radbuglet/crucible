use std::mem::MaybeUninit;

use crate::lang::iter::iter_repeat_len;

use super::transmute::{entirely_unchecked_transmute, sizealign_checked_transmute};

// === Array transposition === //

pub const fn transpose_uninit_array_inner<T, const N: usize>(
	arr: MaybeUninit<[T; N]>,
) -> [MaybeUninit<T>; N] {
	unsafe { sizealign_checked_transmute(arr) }
}

pub const fn transpose_uninit_array_outer<T, const N: usize>(
	arr: [MaybeUninit<T>; N],
) -> MaybeUninit<[T; N]> {
	unsafe { sizealign_checked_transmute(arr) }
}

pub const fn new_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	let arr = MaybeUninit::<[T; N]>::uninit();
	transpose_uninit_array_inner(arr)
}

pub const unsafe fn assume_init_array<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> [T; N] {
	transpose_uninit_array_outer(arr)
		// Safety: provided by caller
		.assume_init()
}

// === Array macro === //

#[doc(hidden)]
pub mod macro_internal {
	use super::*;
	pub use std::mem::MaybeUninit;

	#[repr(C)] // N.B. this is needed for a janky CTFE hack in `unwrap()`.
	pub struct ArrayBuilder<T, const N: usize> {
		pub array: [MaybeUninit<T>; N],
		// Invariant: The first `init_count` of `array` must be initialized.
		pub init_count: usize,
	}

	impl<T, const N: usize> ArrayBuilder<T, N> {
		pub const unsafe fn new() -> Self {
			Self {
				array: new_uninit_array(),
				init_count: 0,
			}
		}

		pub const unsafe fn unwrap(self) -> [T; N] {
			// Safety: `array` is the first element of this `repr(C)` structure so we can transmute
			// an owned instance of the structure into an owned instance of this field. We also
			// perform an implicit transmute from `[MaybeUninit<T>; N]` to `[T; N]`, whose safety is
			// guaranteed by the caller.
			//
			// We do things this way to avoid calling our destructor.
			entirely_unchecked_transmute(self)
		}

		pub const fn len(&self) -> usize {
			N
		}
	}

	impl<T, const N: usize> Drop for ArrayBuilder<T, N> {
		fn drop(&mut self) {
			for i in 0..self.init_count {
				unsafe {
					// Safety: provided during call to `ArrayBuilder::new`.
					self.array[i].assume_init_drop()
				};
			}
		}
	}
}

#[macro_export]
macro_rules! arr {
	($ctor:expr; $size:expr) => {{
		$crate::arr![_ignored => $ctor; $size]
	}};
	($index:ident => $ctor:expr; $size:expr) => {{
		// N.B. const expressions do not inherit the `unsafe` scope from their surroundings.
		let mut arr = unsafe { $crate::mem::array::macro_internal::ArrayBuilder::<_, { $size }>::new() };

		while arr.init_count < arr.len() {
			arr.array[arr.init_count] = $crate::mem::array::macro_internal::MaybeUninit::new({
				let $index = arr.init_count;
				$ctor
			});
			arr.init_count += 1;
		}

		unsafe { arr.unwrap() }
	}};
}

pub use arr;

// === Array constructors === //

pub fn arr_from_iter<T, I: IntoIterator<Item = T>, const N: usize>(iter: I) -> [T; N] {
	let mut iter = iter.into_iter();

	arr![
		i => iter.next()
			.unwrap_or_else(|| panic!("Expected at least {N} element(s); got {}", i));
		N
	]
}

pub fn zip_arr<A, B, const N: usize>(a: [A; N], b: [B; N]) -> [(A, B); N] {
	arr_from_iter(a.into_iter().zip(b.into_iter()))
}

pub fn map_arr<A, B, F, const N: usize>(a: [A; N], f: F) -> [B; N]
where
	F: FnMut(A) -> B,
{
	arr_from_iter(a.into_iter().map(f))
}

// === Boxed array creation === //

pub fn boxed_arr_from_iter<T, const N: usize>(iter: impl IntoIterator<Item = T>) -> Box<[T; N]> {
	Box::new(arr_from_iter(iter))
}

pub fn boxed_arr_from_fn<T, const N: usize>(mut f: impl FnMut() -> T) -> Box<[T; N]> {
	Box::new(arr![f(); N])
}

pub fn boxed_slice_from_fn<T>(f: impl FnMut() -> T, len: usize) -> Box<[T]> {
	iter_repeat_len(f, len).collect()
}
