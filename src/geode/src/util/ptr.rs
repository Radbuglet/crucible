use std::{
	marker::Unsize,
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
