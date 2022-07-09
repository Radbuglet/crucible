use core::mem::{transmute, ManuallyDrop, MaybeUninit};

pub const fn assume_array_init<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> [T; N]

pub const fn new_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	let array = MaybeUninit::<[T; N]>::uninit();
	
}

pub const fn uninit_array_inner<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> MaybeUninit<[T; N]> {
	unsafe { super_unchecked_transmute::<MaybeUninit<[T; N]>, [MaybeUninit<T>; N]>(array) }
}

pub const fn uninit_array_outer<T, const N: usize>(arr: [MaybeUninit<T>; N]) -> MaybeUninit<[T; N]> {
	unsafe { super_unchecked_transmute::<MaybeUninit<[T; N]>, [MaybeUninit<T>; N]>(array) }
}

pub fn big_array<I: IntoIterator, const N: usize>(producer: I) -> [I::Item; N] {
	let mut arr = uninit_array();
	let mut producer = producer.into_iter();

	for slot in &mut arr {
		*slot = MaybeUninit::new(
			producer
				.next()
				.expect("not enough elements to accomodate array"),
		);
	}

	
}
