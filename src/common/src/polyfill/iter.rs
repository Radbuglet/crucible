use std::mem::MaybeUninit;

pub fn decompose_iter<I, T, E, const N: usize>(iter: I) -> Result<[T; N], E>
where
	I: IntoIterator<Item = Result<T, E>>,
{
	let mut array = MaybeUninit::<[T; N]>::uninit();
	let uninit_slice = unsafe { &mut *array.as_mut_ptr().cast::<[MaybeUninit<T>; N]>() };
	let mut i = 0usize;

	for elem in iter {
		if i >= uninit_slice.len() {
			panic!("Too many iterator elements to pack into an array of length {N}.");
		}
		uninit_slice[i].write(elem?);
		i += 1;
	}

	if i < uninit_slice.len() {
		panic!(
			"Expected there to be exactly {} element{} in the iterator but only found {} (missing {})",
			N,
			if N == 1 { "" } else { "s" },
			i,
			N - i,
		);
	}

	Ok(unsafe { array.assume_init() })
}
