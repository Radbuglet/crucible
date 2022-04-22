use crate::util::error::ResultExt;
use std::borrow::Borrow;
use std::error::Error;
use std::hash::Hash;
use std::marker::PhantomData;
use thiserror::Error;

// === Core accessor traits === //

// TrustedEq
pub unsafe trait TrustedEq: Eq {}

macro trusted_eq_for_prim($($ty:ty),*$(,)?) {
	$(unsafe impl TrustedEq for $ty {})*
}

#[rustfmt::skip]
trusted_eq_for_prim!(
	// Numeric primitives
	u8,    i8,
	u16,   i16,
	u32,   i32,
	u64,   i64,
	usize, isize,

	// Simple containers
	&'_ str,
	String,
);

// Accessor constructors
pub trait AsAccessorRef<'a> {
	type Key: TrustedEq;
	type Error: Error;
	type Ref;
	type AccessorRef: Accessor<'a, Key = Self::Key, Error = Self::Error, Ref = Self::Ref>;

	fn accessor_ref(&'a self) -> Untainted<Self::AccessorRef>;

	fn try_get<Q: Borrow<Self::Key>>(&'a self, key: Q) -> Result<Self::Ref, Self::Error> {
		unsafe { self.accessor_ref().unwrap().try_get_unchecked(key) }
	}

	fn get<Q: Borrow<Self::Key>>(&'a self, key: Q) -> Self::Ref {
		self.try_get(key).unwrap_pretty()
	}
}

pub trait AsAccessorMut<'a>: AsAccessorRef<'a> {
	type Mut;
	type AccessorMut: AccessorMut<
		'a,
		Key = Self::Key,
		Error = Self::Error,
		Ref = Self::Ref,
		Mut = Self::Mut,
	>;

	fn accessor_mut(&'a mut self) -> Untainted<Self::AccessorMut>;

	fn try_get_mut<Q: Borrow<Self::Key>>(&'a mut self, key: Q) -> Result<Self::Mut, Self::Error> {
		unsafe { self.accessor_mut().unwrap().try_get_unchecked_mut(key) }
	}

	fn get_mut<Q: Borrow<Self::Key>>(&'a mut self, key: Q) -> Self::Mut {
		self.try_get_mut(key).unwrap_pretty()
	}

	fn try_get_pair_mut<Q: Borrow<Self::Key>, P: Borrow<Self::Key>>(
		&'a mut self,
		key_a: Q,
		key_b: P,
	) -> Result<(Self::Mut, Self::Mut), Self::Error> {
		let (key_a, key_b) = (key_a.borrow(), key_b.borrow());
		if key_a == key_b {
			panic!("Keys cannot alias.");
		}

		let accessor = self.accessor_mut().unwrap();
		unsafe {
			Ok((
				accessor.try_get_unchecked_mut(key_a)?,
				accessor.try_get_unchecked_mut(key_b)?,
			))
		}
	}

	fn get_pair_mut<Q: Borrow<Self::Key>, P: Borrow<Self::Key>>(
		&'a mut self,
		key_a: Q,
		key_b: P,
	) -> (Self::Mut, Self::Mut) {
		self.try_get_pair_mut(key_a, key_b).unwrap_pretty()
	}
}

pub struct Untainted<T>(T);

impl<T> Untainted<T> {
	pub unsafe fn new(value: T) -> Self {
		Self(value)
	}

	pub fn unwrap(self) -> T {
		self.0
	}
}

// Accessors
pub trait Accessor<'a> {
	type Key: TrustedEq;
	type Error: Error;
	type Ref: 'a;

	unsafe fn try_get_unchecked<Q: Borrow<Self::Key>>(
		&self,
		key: Q,
	) -> Result<Self::Ref, Self::Error>;

	unsafe fn get_unchecked<Q: Borrow<Self::Key>>(&self, key: Q) -> Self::Ref {
		self.try_get_unchecked(key).unwrap_pretty()
	}
}

pub trait AccessorMut<'a>: Accessor<'a> {
	type Mut: 'a;

	unsafe fn try_get_unchecked_mut<Q: Borrow<Self::Key>>(
		&self,
		key: Q,
	) -> Result<Self::Mut, Self::Error>;

	unsafe fn get_unchecked_mut<Q: Borrow<Self::Key>>(&self, key: Q) -> Self::Mut {
		self.try_get_unchecked_mut(key).unwrap_pretty()
	}
}

// === Standard accessors === //

// Error types
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
#[error("index {index} out of the slice bounds (length {length})")]
pub struct SliceIndexError {
	index: usize,
	length: usize,
}

// &'a [T] accessor
impl<'a, T: 'a> Accessor<'a> for &'a [T] {
	type Key = usize;
	type Error = SliceIndexError;
	type Ref = &'a T;

	unsafe fn try_get_unchecked<Q: Borrow<Self::Key>>(
		&self,
		key: Q,
	) -> Result<Self::Ref, Self::Error> {
		let index = *key.borrow();
		self.get(index).ok_or(SliceIndexError {
			index,
			length: self.len(),
		})
	}
}

// &'a mut [T] accessor
pub struct SliceAccessorMut<'a, T> {
	_ty: PhantomData<&'a mut [T]>,
	base: *mut T,
	length: usize,
}

impl<'a, T> SliceAccessorMut<'a, T> {
	pub fn new(slice: &'a mut [T]) -> Self {
		let length = slice.len();
		Self {
			_ty: PhantomData,
			base: slice.as_mut_ptr(),
			length,
		}
	}
}

impl<'a, T: 'a> Accessor<'a> for SliceAccessorMut<'a, T> {
	type Key = usize;
	type Error = SliceIndexError;
	type Ref = &'a T;

	unsafe fn try_get_unchecked<Q: Borrow<Self::Key>>(
		&self,
		key: Q,
	) -> Result<Self::Ref, Self::Error> {
		let index = *key.borrow();
		if index < self.length {
			Ok(&*self.base.add(index))
		} else {
			Err(SliceIndexError {
				index,
				length: self.length,
			})
		}
	}
}

impl<'a, T: 'a> AccessorMut<'a> for SliceAccessorMut<'a, T> {
	type Mut = &'a mut T;

	unsafe fn try_get_unchecked_mut<Q: Borrow<Self::Key>>(
		&self,
		key: Q,
	) -> Result<Self::Mut, Self::Error> {
		let index = *key.borrow();
		if index < self.length {
			Ok(&mut *self.base.add(index))
		} else {
			Err(SliceIndexError {
				index,
				length: self.length,
			})
		}
	}
}

// Constructors
impl<'a, T: 'a> AsAccessorRef<'a> for [T] {
	type Key = usize;
	type Error = SliceIndexError;
	type Ref = &'a T;
	type AccessorRef = &'a [T];

	fn accessor_ref(&'a self) -> Untainted<Self::AccessorRef> {
		unsafe { Untainted::new(self) }
	}
}

impl<'a, T: 'a> AsAccessorMut<'a> for [T] {
	type Mut = &'a mut T;
	type AccessorMut = SliceAccessorMut<'a, T>;

	fn accessor_mut(&'a mut self) -> Untainted<Self::AccessorMut> {
		unsafe { Untainted::new(SliceAccessorMut::new(self)) }
	}
}

// === Tests === //

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn swaps() {
		let mut target = vec![0, 1, 2];
		let (a, b) = target.get_pair_mut(1, 2);
		std::mem::swap(a, b);
		assert_eq!(target, &[0, 2, 1]);
	}
}
