use std::{alloc::Layout, marker::PhantomData};

use crate::{error::ResultExt, layout::ByteCounter};

pub struct PerfectMap<T> {
	_ty: PhantomData<T>,
}

impl<T> PerfectMap<T> {
	pub fn new<I>(iter: I) -> Self
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		// Deconstruct iterator
		let elem_iter = iter.into_iter();
		let elem_count = elem_iter.len();

		// Determine heuristic size of map
		let bucket_layout = Layout::new::<*mut [T]>();
		let elem_layout = Layout::new::<T>();

		let mut size = ByteCounter::default();

		// Invalidation is done here because `ByteCounter` has branchless invalidation.
		size.invalidate_if_greater_than_isize_max(elem_count);

		// We put buckets first because they are aligned to `align_of::<usize>()`, which is usually
		// the platform's highest natural alignment.
		size.bump_array(bucket_layout, elem_count);

		// Experimentally, the cell count load factor for perfect `HashMaps` with a maximum rehash
		// count of 50 is at most `2.01`. Since the number of buckets is constant, this means that
		// the load factor for elements is `1.01`. We'll approximate that to `1.125` and compute it
		// as `n * 1.125 = n + n / 8`.
		size.bump_array(elem_layout, elem_count + elem_count / 8);

		let size = size.size().unwrap_pretty();

		todo!()
	}
}

struct Bucket<T> {
	values: *mut [T],
	rng: usize,
}
