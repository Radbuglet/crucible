use crate::util::error::ResultExt;
use derive_where::derive_where;
use std::borrow::Borrow;
use std::error::Error;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;
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

// Bounded
pub unsafe trait Bounded<'r> {}

unsafe impl<'a: 'r, 'r, T: ?Sized> Bounded<'r> for &'a T {}
unsafe impl<'a: 'r, 'r, T: ?Sized> Bounded<'r> for &'a mut T {}

// Untainted
pub trait MutabilityMarker {}

pub struct Ref {
	_private: (),
}

impl MutabilityMarker for Ref {}

pub struct Mut {
	_private: (),
}

impl MutabilityMarker for Mut {}

pub type UntaintedRef<T> = Untainted<T, Ref>;

pub type UntaintedMut<T> = Untainted<T, Mut>;

/// An opaque wrapper that asserts that a given [Accessor] is untainted. This means that, immediately
/// upon [`.unwrap`](unwrap)'ing the wrapper, one can assume that every value in the `Accessor` is
/// borrowable by the exposed [AccessorRef::try_get_unchecked] and [AccessorRef::try_get_unchecked_mut]
/// (if `M = Mut`) methods.
pub struct Untainted<T: Accessor, M: MutabilityMarker> {
	_ty: PhantomData<fn(M) -> M>,
	value: T,
}

impl<T: Accessor, M: MutabilityMarker> Untainted<T, M> {
	/// Wraps an [Accessor] to mark it as "untainted."
	///
	/// ## Safety
	///
	/// See structure item's documentation on the definition of "untainted."
	pub unsafe fn new(value: T) -> Self {
		Self {
			_ty: PhantomData,
			value,
		}
	}

	/// Unwraps an [Accessor] and asserts the invariants provided by the structure's item
	/// documentation.
	pub fn unwrap(self) -> T {
		self.value
	}

	pub fn as_ref(&self) -> UntaintedRef<&T> {
		unsafe {
			// Safety:
			// We can already assume that the value we contain is immutably untainted by the invariant
			// of the structure. We know that this method doesn't mess up this invariant because
			// references are limited to the lifetime of `&T` and, while we return `UntaintedRef<&T>`
			// instances, we prevent `.unwrap()` and `.as_mut()` calls.
			UntaintedRef::new(&self.value)
		}
	}

	/// Unwraps an [Accessor] and asserts the **immutable** invariants provided by the structure's item
	/// documentation. This is equivalent to `.as_ref().unwrap()`
	pub fn unwrap_ref(&self) -> &T {
		self.as_ref().unwrap()
	}
}

impl<T: Accessor> UntaintedMut<T> {
	pub fn as_mut(&mut self) -> UntaintedMut<&T> {
		unsafe {
			// Safety:
			// We can already assume that the value we contain is mutably untainted by the invariants
			// of the structure. We know that this method doesn't mess up this invariant because
			// the references are limited to the lifetime of `&T` and, while we return `UntaintedMut<&T>`
			// instances, we prevent `.unwrap()` and `.as_mut()` calls.
			UntaintedMut::new(&self.value)
		}
	}

	/// Unwraps an [Accessor] and asserts the invariants provided by the structure's item documentation.
	/// This is equivalent to `.as_mut().unwrap()`
	pub fn unwrap_mut(&mut self) -> &T {
		self.as_mut().unwrap()
	}
}

impl<T: Accessor, M: MutabilityMarker> ToAccessor for Untainted<T, M> {
	type Accessor = T;
	type Marker = M;

	fn to_accessor(self) -> Untainted<Self::Accessor, Self::Marker> {
		self
	}
}

// Accessor
pub trait ToAccessor {
	type Accessor: Accessor;
	type Marker: MutabilityMarker;

	fn to_accessor(self) -> Untainted<Self::Accessor, Self::Marker>;
}

pub trait Accessor: Clone {
	type Key: TrustedEq;
	type Error: Error;
}

pub trait AccessorRef<'r>: 'r + Accessor {
	type Ref: Bounded<'r> + Clone;

	unsafe fn try_get_unchecked<Q>(&'r self, key: Q) -> Result<Self::Ref, Self::Error>
	where
		Q: Borrow<Self::Key>;

	unsafe fn get_unchecked<Q: Borrow<Self::Key>>(&'r self, key: Q) -> Self::Ref {
		self.try_get_unchecked(key).unwrap_pretty()
	}
}

pub trait AccessorMut<'r>: AccessorRef<'r> {
	type Mut: Bounded<'r>;

	unsafe fn try_get_unchecked_mut<Q>(&'r self, key: Q) -> Result<Self::Mut, Self::Error>
	where
		Q: Borrow<Self::Key>;

	unsafe fn get_unchecked_mut<Q: Borrow<Self::Key>>(&'r self, key: Q) -> Self::Mut {
		// Safety: provided by caller
		self.try_get_unchecked_mut(key).unwrap_pretty()
	}
}

impl<T: Clone + Deref<Target = A>, A: Accessor> Accessor for T {
	type Key = A::Key;
	type Error = A::Error;
}

impl<'r, T: 'r + Clone + Deref<Target = A>, A: AccessorRef<'r>> AccessorRef<'r> for T {
	type Ref = A::Ref;

	unsafe fn try_get_unchecked<Q>(&'r self, key: Q) -> Result<Self::Ref, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		// Safety is provided by caller.
		// `Deref` is not allowed to run unchecked borrows in the meantime since
		// it lacks the proper guarantees to do so.
		(&**self).try_get_unchecked(key)
	}
}

impl<'r, T: 'r + Clone + Deref<Target = A>, A: AccessorMut<'r>> AccessorMut<'r> for T {
	type Mut = A::Mut;

	unsafe fn try_get_unchecked_mut<Q>(&'r self, key: Q) -> Result<Self::Mut, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		// Safety is provided by caller.
		// `Deref` is not allowed to run unchecked borrows in the meantime since
		// it lacks the proper guarantees to do so.
		(&**self).try_get_unchecked_mut(key)
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

#[derive(Debug)]
#[derive_where(Copy, Clone)]
pub struct SliceAccessorRef<'a, T>(&'a [T]);

impl<'a, T> ToAccessor for &'a [T] {
	type Accessor = SliceAccessorRef<'a, T>;
	type Marker = Ref;

	fn to_accessor(self) -> UntaintedRef<Self::Accessor> {
		unsafe { UntaintedRef::new(SliceAccessorRef(self)) }
	}
}

impl<'m, T: 'm> Accessor for SliceAccessorRef<'m, T> {
	type Key = usize;
	type Error = SliceIndexError;
}

// &'a [T] accessor
impl<'r, 'm: 'r, T: 'm> AccessorRef<'r> for SliceAccessorRef<'m, T> {
	type Ref = &'r T;

	unsafe fn try_get_unchecked<Q>(&'r self, key: Q) -> Result<Self::Ref, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		let index = *key.borrow();

		self.0.get(index).ok_or(SliceIndexError {
			index,
			length: self.0.len(),
		})
	}
}

// &'a mut [T] accessor
#[derive_where(Clone)]
pub struct SliceAccessorMut<'a, T> {
	_ty: PhantomData<&'a mut [T]>,
	base: *mut T,
	length: usize,
}

impl<'a, T> ToAccessor for &'a mut [T] {
	type Accessor = SliceAccessorMut<'a, T>;
	type Marker = Mut;

	fn to_accessor(self) -> UntaintedMut<Self::Accessor> {
		let length = self.len();
		unsafe {
			Untainted::new(SliceAccessorMut {
				_ty: PhantomData,
				base: self.as_mut_ptr(),
				length,
			})
		}
	}
}

impl<'m, T> Accessor for SliceAccessorMut<'m, T> {
	type Key = usize;
	type Error = SliceIndexError;
}

impl<'r, 'm: 'r, T: 'm> AccessorRef<'r> for SliceAccessorMut<'m, T> {
	type Ref = &'r T;

	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<Self::Ref, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
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

impl<'r, 'm: 'r, T: 'm> AccessorMut<'r> for SliceAccessorMut<'m, T> {
	type Mut = &'r mut T;

	unsafe fn try_get_unchecked_mut<Q>(&self, key: Q) -> Result<Self::Mut, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
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

// === Extensions === //

impl<'r, A: AccessorRef<'r>, M: MutabilityMarker> Untainted<A, M> {
	pub fn try_get<Q>(&'r self, key: Q) -> Result<A::Ref, A::Error>
	where
		Q: Borrow<A::Key>,
	{
		unsafe { self.unwrap_ref().try_get_unchecked(key) }
	}

	pub fn get<Q>(&'r self, key: Q) -> A::Ref
	where
		Q: Borrow<A::Key>,
	{
		self.try_get(key).unwrap_pretty()
	}
}

impl<'r, A: AccessorMut<'r>> UntaintedMut<A> {
	pub fn try_get_mut<Q>(&'r mut self, key: Q) -> Result<A::Mut, A::Error>
	where
		Q: Borrow<A::Key>,
	{
		unsafe { self.unwrap_mut().try_get_unchecked_mut(key) }
	}

	pub fn get_mut<Q>(&'r mut self, key: Q) -> A::Mut
	where
		Q: Borrow<A::Key>,
	{
		self.try_get_mut(key).unwrap_pretty()
	}

	pub fn try_get_pair_mut<Q, P>(&'r mut self, a: Q, b: P) -> Result<(A::Mut, A::Mut), A::Error>
	where
		Q: Borrow<A::Key>,
		P: Borrow<A::Key>,
	{
		let (a, b) = (a.borrow(), b.borrow());
		let accessor = self.unwrap_mut();
		if a == b {
			panic!("Keys cannot alias!");
		}
		unsafe {
			Ok((
				accessor.try_get_unchecked_mut(a)?,
				accessor.try_get_unchecked_mut(a)?,
			))
		}
	}

	pub fn get_pair_mut<Q, P>(&'r mut self, a: Q, b: P) -> (A::Mut, A::Mut)
	where
		Q: Borrow<A::Key>,
		P: Borrow<A::Key>,
	{
		self.try_get_pair_mut(a, b).unwrap_pretty()
	}
}

// === Tests === //

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn swaps() {
		let mut target = vec![0, 1, 2];
		let mut target_proxy = target.as_mut_slice().to_accessor();
		let (a, b) = target_proxy.get_pair_mut(1, 2);
		std::mem::swap(a, b);
		assert_eq!(target, &[0, 2, 1]);
	}
}
