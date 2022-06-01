// Welcome to the single most rewritten system of the entire project.

use crate::util::error::ResultExt;
use derive_where::derive_where;
use std::borrow::Borrow;
use std::error::Error;
use std::fmt::Debug;
use std::marker::PhantomData;
use thiserror::Error;

// === TrustedEq === //

pub unsafe trait TrustedEq: Eq {}

unsafe impl TrustedEq for u8 {}
unsafe impl TrustedEq for i8 {}
unsafe impl TrustedEq for u16 {}
unsafe impl TrustedEq for i16 {}
unsafe impl TrustedEq for u32 {}
unsafe impl TrustedEq for i32 {}
unsafe impl TrustedEq for u64 {}
unsafe impl TrustedEq for i64 {}
unsafe impl TrustedEq for usize {}
unsafe impl TrustedEq for isize {}

// === ToAccessor === //

pub trait ToAccessor: Sized {
	type Accessor: AccessorBase;

	fn to_accessor(self) -> Untainted<Self::Accessor>;
}

pub struct Untainted<T: AccessorBase>(T);

impl<T: AccessorBase> Untainted<T> {
	pub unsafe fn new(value: T) -> Self {
		Self(value)
	}

	pub fn unwrap(self) -> T {
		self.0
	}

	pub fn borrow_ref_container(&self) -> Untainted<AccessorRefProxy<'_, T>> {
		self.to_accessor()
	}

	pub fn borrow_mut_container(&mut self) -> Untainted<AccessorMutProxy<'_, T>> {
		self.to_accessor()
	}

	pub fn to_refs(self) -> AccessorRefWrapper<T> {
		AccessorRefWrapper::new(self)
	}
}

impl<T: AccessorBase> ToAccessor for Untainted<T> {
	type Accessor = T;

	fn to_accessor(self) -> Untainted<Self::Accessor> {
		self
	}
}

impl<'m, T: AccessorBase> ToAccessor for &'m Untainted<T> {
	type Accessor = AccessorRefProxy<'m, T>;

	fn to_accessor(self) -> Untainted<Self::Accessor> {
		unsafe { Untainted::new(AccessorRefProxy::new(&self.0)) }
	}
}

impl<'m, T: AccessorBase> ToAccessor for &'m mut Untainted<T> {
	type Accessor = AccessorMutProxy<'m, T>;

	fn to_accessor(self) -> Untainted<Self::Accessor> {
		unsafe { Untainted::new(AccessorMutProxy::new(&self.0)) }
	}
}

// === Accessor === //

pub trait AccessorBase: Clone {
	type Key: Debug + TrustedEq + Clone;
	type Value: ?Sized;
	type Error: Error;
}

pub trait Accessor<'r>: AccessorBase {
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>;

	unsafe fn get_unchecked<Q>(&self, key: Q) -> &'r Self::Value
	where
		Q: Borrow<Self::Key>,
	{
		self.try_get_unchecked(key).unwrap_pretty()
	}
}

pub trait AccessorMut<'r>: Accessor<'r> {
	unsafe fn try_get_unchecked_mut<Q>(&self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>;

	unsafe fn get_unchecked_mut<Q>(&self, key: Q) -> &'r mut Self::Value
	where
		Q: Borrow<Self::Key>,
	{
		self.try_get_unchecked_mut(key).unwrap_pretty()
	}
}

// === Accessor Proxies === //

#[derive_where(Clone)]
pub struct AccessorRefProxy<'r, A>(&'r A);

impl<'r, A> AccessorRefProxy<'r, A> {
	pub fn new(accessor: &'r A) -> Self {
		Self(accessor)
	}
}

impl<'r, A: AccessorBase> AccessorBase for AccessorRefProxy<'r, A> {
	type Key = A::Key;
	type Value = A::Value;
	type Error = A::Error;
}

impl<'i: 'r, 'r, A: Accessor<'i>> Accessor<'r> for AccessorRefProxy<'r, A>
where
	A::Value: 'i,
{
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		self.0.try_get_unchecked(key)
	}
}

#[derive_where(Clone)]
pub struct AccessorMutProxy<'r, A>(&'r A);

impl<'r, A> AccessorMutProxy<'r, A> {
	pub fn new(accessor: &'r A) -> Self {
		Self(accessor)
	}
}

impl<'r, A: AccessorBase> AccessorBase for AccessorMutProxy<'r, A> {
	type Key = A::Key;
	type Value = A::Value;
	type Error = A::Error;
}

impl<'i: 'r, 'r, A: Accessor<'i>> Accessor<'r> for AccessorMutProxy<'r, A>
where
	A::Value: 'i,
{
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		self.0.try_get_unchecked(key)
	}
}

impl<'i: 'r, 'r, A: AccessorMut<'i>> AccessorMut<'r> for AccessorMutProxy<'r, A>
where
	A::Value: 'i,
{
	unsafe fn try_get_unchecked_mut<Q>(&self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		self.0.try_get_unchecked_mut(key)
	}
}

#[derive(Clone)]
pub struct AccessorRefWrapper<A>(A);

impl<A: AccessorBase> AccessorRefWrapper<A> {
	pub fn new(accessor: Untainted<A>) -> Self {
		Self(accessor.unwrap())
	}
}

impl<'r, A: Accessor<'r>> AccessorRefWrapper<A> {
	pub fn try_get<Q>(&self, key: Q) -> Result<&'r A::Value, A::Error>
	where
		Q: Borrow<A::Key>,
	{
		unsafe { self.0.try_get_unchecked(key) }
	}

	pub fn get<Q>(&self, key: Q) -> &'r A::Value
	where
		Q: Borrow<A::Key>,
	{
		self.try_get(key).unwrap_pretty()
	}
}

// === Standard accessors === //

// Error types
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
#[error("index {index} out of slice bounds (length {length})")]
pub struct SliceIndexError {
	index: usize,
	length: usize,
}

impl<'r, T> ToAccessor for &'r [T] {
	type Accessor = &'r [T];

	fn to_accessor(self) -> Untainted<Self::Accessor> {
		unsafe { Untainted::new(self) }
	}
}

impl<'r, T> AccessorBase for &'r [T] {
	type Key = usize;
	type Value = T;
	type Error = SliceIndexError;
}

// &'r [T] accessor
impl<'r, T> Accessor<'r> for &'r [T] {
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		let index = *key.borrow();
		self.get(index).ok_or(SliceIndexError {
			index,
			length: self.len(),
		})
	}
}

// &'r mut [T] accessor
#[derive_where(Clone)]
pub struct SliceAccessorMut<'r, T> {
	_ty: PhantomData<&'r mut [T]>,
	base: *mut T,
	length: usize,
}

impl<'r, T> ToAccessor for &'r mut [T] {
	type Accessor = SliceAccessorMut<'r, T>;

	fn to_accessor(self) -> Untainted<Self::Accessor> {
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

impl<'r, T> AccessorBase for SliceAccessorMut<'r, T> {
	type Key = usize;
	type Value = T;
	type Error = SliceIndexError;
}

impl<'r, T> Accessor<'r> for SliceAccessorMut<'r, T> {
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
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

impl<'r, T> AccessorMut<'r> for SliceAccessorMut<'r, T> {
	unsafe fn try_get_unchecked_mut<Q>(&self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
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

// === Extension methods === //

// TODO: Implement `BorrowedAccessorExt` methods.

pub trait OwnedAccessorRefExt<'r> {
	type Key;
	type Value: ?Sized;
	type Error;
	type Accessor: AccessorBase;

	fn try_take<Q>(self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>;

	fn take<Q>(self, key: Q) -> &'r Self::Value
	where
		Q: Borrow<Self::Key>;
}

pub trait OwnedAccessorMutExt<'r>: OwnedAccessorRefExt<'r> {
	fn try_take_mut<Q>(self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>;

	fn take_mut<Q>(self, key: Q) -> &'r mut Self::Value
	where
		Q: Borrow<Self::Key>;

	fn try_take_pair_mut<Q, P>(
		self,
		a: Q,
		b: P,
	) -> Result<(&'r mut Self::Value, &'r mut Self::Value), Self::Error>
	where
		Q: Borrow<Self::Key>,
		P: Borrow<Self::Key>;

	fn take_pair_mut<Q, P>(self, a: Q, b: P) -> (&'r mut Self::Value, &'r mut Self::Value)
	where
		Q: Borrow<Self::Key>,
		P: Borrow<Self::Key>;

	#[allow(clippy::type_complexity)] // there's not really a way to simplify this type
	fn try_take_exclude_mut<Q>(
		self,
		key: Q,
	) -> Result<(&'r mut Self::Value, Untainted<ExcludeOne<Self::Accessor>>), Self::Error>
	where
		Q: Borrow<Self::Key>;

	fn take_exclude_mut<Q>(
		self,
		key: Q,
	) -> (&'r mut Self::Value, Untainted<ExcludeOne<Self::Accessor>>)
	where
		Q: Borrow<Self::Key>;
}

impl<'r, C: ToAccessor<Accessor = A>, A: Accessor<'r>> OwnedAccessorRefExt<'r> for C {
	type Key = A::Key;
	type Value = A::Value;
	type Error = A::Error;
	type Accessor = A;

	fn try_take<Q>(self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		unsafe { self.to_accessor().unwrap().try_get_unchecked(key) }
	}

	fn take<Q>(self, key: Q) -> &'r Self::Value
	where
		Q: Borrow<Self::Key>,
	{
		self.try_take(key).unwrap_pretty()
	}
}

impl<'r, C: ToAccessor<Accessor = A>, A: AccessorMut<'r>> OwnedAccessorMutExt<'r> for C {
	fn try_take_mut<Q>(self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		unsafe { self.to_accessor().unwrap().try_get_unchecked_mut(key) }
	}

	fn take_mut<Q>(self, key: Q) -> &'r mut Self::Value
	where
		Q: Borrow<Self::Key>,
	{
		self.try_take_mut(key).unwrap_pretty()
	}

	fn try_take_pair_mut<Q, P>(
		self,
		a: Q,
		b: P,
	) -> Result<(&'r mut Self::Value, &'r mut Self::Value), Self::Error>
	where
		Q: Borrow<Self::Key>,
		P: Borrow<Self::Key>,
	{
		let accessor = self.to_accessor().unwrap();
		let (a, b) = (a.borrow(), b.borrow());
		assert_ne!(a, b, "keys cannot alias");

		unsafe {
			Ok((
				accessor.try_get_unchecked_mut(a)?,
				accessor.try_get_unchecked_mut(b)?,
			))
		}
	}

	fn take_pair_mut<Q, P>(self, a: Q, b: P) -> (&'r mut Self::Value, &'r mut Self::Value)
	where
		Q: Borrow<Self::Key>,
		P: Borrow<Self::Key>,
	{
		self.try_take_pair_mut(a, b).unwrap_pretty()
	}

	fn try_take_exclude_mut<Q>(
		self,
		key: Q,
	) -> Result<(&'r mut Self::Value, Untainted<ExcludeOne<Self::Accessor>>), Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		let key = key.borrow();
		let accessor = self.to_accessor().unwrap();

		unsafe {
			let removed = accessor.try_get_unchecked_mut(key)?;
			let other = Untainted::new(ExcludeOne {
				accessor,
				excluded_index: key.clone(),
			});

			Ok((removed, other))
		}
	}

	fn take_exclude_mut<Q>(
		self,
		key: Q,
	) -> (&'r mut Self::Value, Untainted<ExcludeOne<Self::Accessor>>)
	where
		Q: Borrow<Self::Key>,
	{
		self.try_take_exclude_mut(key).unwrap_pretty()
	}
}

// pub fn swap_pair<'r, C, A, Q, P>(container: C, a: Q, b: P)
// where
// 	C: ToAccessor<Accessor = A>,
// 	A: AccessorMut<'r>,
// 	A::Value: 'r + Sized,
// 	Q: Borrow<A::Key>,
// 	P: Borrow<A::Key>,
// {
// 	let (a, b) = get_pair_mut(container, a, b);
// 	std::mem::swap(a, b);
// }

// === Views === //

#[derive(Debug, Copy, Clone, Error)]
pub enum ExcludeOneError<K: Debug, E: Error> {
	#[error("attempted to access excluded entry with key {0:?}")]
	Hole(K),
	#[error(transparent)]
	Underlying(#[from] E),
}

#[derive(Clone)]
pub struct ExcludeOne<A: AccessorBase> {
	accessor: A,
	excluded_index: A::Key,
}

impl<A: AccessorBase> AccessorBase for ExcludeOne<A> {
	type Key = A::Key;
	type Value = A::Value;
	type Error = ExcludeOneError<A::Key, A::Error>;
}

impl<'i: 'r, 'r, A: Accessor<'i>> Accessor<'r> for ExcludeOne<A>
where
	A::Value: 'i,
{
	unsafe fn try_get_unchecked<Q>(&self, key: Q) -> Result<&'r Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		let key = key.borrow();
		if key == &self.excluded_index {
			return Err(ExcludeOneError::Hole(key.clone()));
		}

		// We use `Ok(...?)` syntax to automatically convert user errors to `ExcludeOneError`'s.
		Ok(self.accessor.try_get_unchecked(key)?)
	}
}

impl<'i: 'r, 'r, A: AccessorMut<'i>> AccessorMut<'r> for ExcludeOne<A>
where
	A::Value: 'i,
{
	unsafe fn try_get_unchecked_mut<Q>(&self, key: Q) -> Result<&'r mut Self::Value, Self::Error>
	where
		Q: Borrow<Self::Key>,
	{
		let key = key.borrow();
		if key == &self.excluded_index {
			return Err(ExcludeOneError::Hole(key.clone()));
		}

		Ok(self.accessor.try_get_unchecked_mut(key)?)
	}
}

// === Tests === //

#[cfg(test)]
mod tests {
	use super::*;

	fn get_first_three(slice: &mut [i32]) -> (&mut i32, &mut i32, &i32, &i32) {
		let (a, right) = slice.take_exclude_mut(0);
		let (b, mut right) = right.take_exclude_mut(1);

		let (k1, k2) = (&mut right).take_pair_mut(3, 4);
		std::mem::swap(k1, k2);

		let refs = right.to_refs();
		(a, b, refs.get(2), refs.get(3))
	}

	#[test]
	fn swaps() {
		let mut target = vec![0, 1, 2, 3, 4];
		let (a, b, c, d) = get_first_three(&mut target);
		std::mem::swap(a, b);
		assert_eq!(*c, 2);
		assert_eq!(*d, 4);
		assert_eq!(target.as_slice(), [1, 0, 2, 4, 3]);
	}
}
