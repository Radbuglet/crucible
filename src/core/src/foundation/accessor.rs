use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

// === Core traits === //

// FIXME: Concurrent borrowing semantics are, believe it or not, still garbage.

/// An `Accessor` represents an object which maps indices to distinct value pointers in a one-to-one
/// fashion. A vector mapping indices to elements or a hash map mapping keys to values are examples
/// of an `Accessor`. `Accessors` return [PointerLike] objects, which allows users to unsafely promote
/// the pointee to either a mutable or immutable reference. Wrappers and extension methods can use the
/// one-to-one property of the map alongside `PointerLike` promotion to implement mechanisms to provide
/// mutable references to several distinct values in the `Accessor` at once.
///
/// ## Avoiding Wrapper Chains
///
/// To avoid cascades of `Option<Option<Option<NonNull<T>>>>` when wrapping `Accessors` with out-of-
/// bounds reporting, all accessors have the ability to return an `OobError` type implementing the
/// standard library's [Error] trait. Implementors of this trait are recommended to keep error
/// conditions to the `Err` variant of the result, preferring *e.g.* `Err(FirstWrapperError::Parent(
/// SecondWrapperError))` over `Ok(Err(SecondWrapperError))` to make pointer unwrapping always exactly
/// one step.
///
/// While many wrappers return the underlying pointer/reference type directly, some may want to use
/// smart pointers to implement more advanced borrow checking. This, however, runs the risk of making
/// it difficult to access the inner value. To avoid this, `Accessors` also specify a `Self::Value`
/// type, and all references to that value produced by `Self::Ptr` must implement `Deref<Target = Value>`
/// (and `DerefMut` if they're a mutable reference). This has the added benefit of making it easy to
/// accept an abstract `Accessor` where `Value = T` and use the references directly without knowing
/// the underlying target type.
///
/// ## Derived Implementations
///
/// Both mutable and immutable references to [Accessor]s also implement [Accessor].
///
/// ## Safety
///
/// For a given object `T` implementing the `Accessor` trait, the following can be assumed about the
/// instance:
///
/// 1. Entries can be either unborrowed, immutably borrowed, or mutably borrowed. There may only be
///    one exclusive mutable borrow at a given time but there may be an unlimited number of concurrent
///    immutable borrows.
/// 2. If two indices are reported to unequal through [Eq], the returned pointers are guaranteed not
///    to logically alias. In other words, this map is injective.
///    TODO: Replace with TrustedEq and TrustedOrd traits.
/// 3. So long as the virtual aliasing rules described by this safety section are followed properly,
///    returned pointers will be valid to promote for at most the lifetime of the **owned** instance
///    `T`. Note that this implicitly means that owning a `T` reference (`&'a T`) will limit promotion
///    lifetimes to the lifetime of that reference (`'a`), not the lifetime of `T`.
///
/// As a note to implementors, because `Accessors` take immutable references to themselves, promoting
/// its contents to a mutable reference directly may cause undefined behavior unless the target exhibits
/// proper interior mutability with [UnsafeCell](std::cell::UnsafeCell) (or its derivatives) or stores
/// a raw pointer to the underlying mutable target such that it does not conflict with any existing
/// references.
///
/// Also note that this trait makes no guarantees about outstanding borrows for any given instance.
/// Even when holding ownership of `T`, one cannot assume that an external actor has not already
/// borrowed the contents of a derived instance (remember, `&T: Accessor`). Such guarantees must be
/// made by external actors.
pub unsafe trait Accessor {
	type Index: Debug + Copy + Eq;
	type Value: ?Sized;

	type Ptr: PointerLike;
	type OobError: Error;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError>;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		self.try_get_raw(index).unwrap()
	}
}

unsafe impl<'a, T: ?Sized + Accessor> Accessor for &'a T {
	type Index = T::Index;
	type Value = T::Value;
	type Ptr = T::Ptr;
	type OobError = T::OobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, T::OobError> {
		(**self).try_get_raw(index)
	}
}

unsafe impl<'a, T: ?Sized + Accessor> Accessor for &'a mut T {
	type Index = T::Index;
	type Value = T::Value;
	type Ptr = T::Ptr;
	type OobError = T::OobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, T::OobError> {
		(**self).try_get_raw(index)
	}
}

/// Strips an [Accessor] of its [AssertUnborrowed] guarantees, simplifying soundness proofs for that
/// trait.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MaybeBorrowed<T>(T);

impl<T> MaybeBorrowed<T> {
	/// Wraps the `value` in a `MaybeBorrowed` instance, ensuring that users can no longer call
	/// [AssertUnborrowed::assert_unborrowed].
	pub fn new(value: T) -> Self {
		Self(value)
	}

	/// Wraps the `value` reference in a `MaybeBorrowed` instance, ensuring that users can no longer
	/// call [AssertUnborrowed::assert_unborrowed_mut].
	pub fn new_mut(value: &mut T) -> &mut Self {
		unsafe { &mut *(value as *mut T as *mut MaybeBorrowed<T>) }
	}

	/// Asserts that the inner value is unborrowed as allows users to recover the inner value.
	pub unsafe fn unwrap(self) -> T {
		self.0
	}
}

// TODO: This could be replaced with the blanket `SafeDeref` impl on `Accessors` once that's defined.
unsafe impl<T: Accessor> Accessor for MaybeBorrowed<T> {
	type Index = T::Index;
	type Value = T::Value;
	type Ptr = T::Ptr;
	type OobError = T::OobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
		self.0.try_get_raw(index)
	}
}

impl<T> Deref for MaybeBorrowed<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> DerefMut for MaybeBorrowed<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

/// Allows users to assert that all entries in an [Accessor] are currently unborrowed, consuming it
/// and producing a [MaybeBorrowed] wrapper.
///
/// ## Derived Implementations
///
/// Mutable references to objects implementing `AssertUnborrowed` also implement this trait. This is
/// sound because mutable references ensure that the `assert_unborrowed` method of the underlying
/// object cannot be called.
///
/// ## Safety
///
/// Implementors must guarantee that when this conversion is performed, the wrapped value will begin
/// in a state where none of the [Accessor] entries are borrowed.
pub unsafe trait AssertUnborrowed: Sized + Accessor {
	fn assert_unborrowed(self) -> MaybeBorrowed<Self> {
		MaybeBorrowed::new(self)
	}

	fn assert_unborrowed_mut(&mut self) -> &mut MaybeBorrowed<Self> {
		MaybeBorrowed::new_mut(self)
	}
}

unsafe impl<'a, T: AssertUnborrowed> AssertUnborrowed for &'a mut T {}

/// Asserts that every entry within an [Accessor] is logically borrowed for the accessor's lifetime,
/// ensuring that all promotions to immutable references are valid.
///
/// ## Derived Implementations
///
/// Both mutable and immutable references to objects implementing `AssertImmutable` also implement
/// this trait. This is sound because these references live at most as long as their underlying objects
/// and can therefore also provide the same guarantees.
///
/// ## Safety
///
/// Implementors must guarantee that, for as long as `Accessors` implementing this trait are alive,
/// all entries in the underlying `Accessor` must be logically immutably borrowed and must therefore
/// always be valid to promote immutably.
pub unsafe trait AssertImmutable: Accessor {}

unsafe impl<'a, T: AssertImmutable> AssertImmutable for &'a T {}
unsafe impl<'a, T: AssertImmutable> AssertImmutable for &'a mut T {}

/// A raw reference type that can be promoted into either its mutable or immutable form.
///
/// ## Safety
///
/// [PointerLike]s carry no safety guarantees about promotion validity by themselves, and their
/// semantics must typically be augmented by some external contract.
#[rustfmt::skip]
pub trait PointerLike {
	type Value: ?Sized;

	// `Self: 'a` provides a concise (albeit overly-conservative) way of ensuring that the pointee
	// lives as long as the lifetime since objects can only live as long as the lifetimes of their
	// generic parameters.
	#[rustfmt::skip]
	type AsRef<'a>: Deref<Target = Self::Value> where Self: 'a;

	#[rustfmt::skip]
	type AsMut<'a>: Deref<Target = Self::Value> + DerefMut where Self: 'a;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a>;
	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a>;
}

// === Core PointerLike impls === //

#[rustfmt::skip]
impl<T: ?Sized> PointerLike for NonNull<T> {
	type Value = T;

	type AsRef<'a> where Self: 'a = &'a T;
	type AsMut<'a> where Self: 'a = &'a mut T;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a> {
		self.as_ref()
	}

	unsafe fn promote_mut<'a>(mut self) -> Self::AsMut<'a> {
		self.as_mut()
	}
}

#[rustfmt::skip]
impl<'r, T: ?Sized> PointerLike for &'r UnsafeCell<T> {
	type Value = T;

	type AsRef<'a> where Self: 'a = &'a T;
	type AsMut<'a> where Self: 'a = &'a mut T;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a> {
		&*self.get()
	}

	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a> {
		&mut *self.get()
	}
}

// === Core Accessor impls === //

unsafe impl<T> AssertImmutable for [T] {}

unsafe impl<T> Accessor for [T] {
	type Index = usize;
	type Value = T;
	type Ptr = NonNull<T>;
	type OobError = SliceOobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
		match self.get(index) {
			Some(elem) => Ok(NonNull::from(elem)),
			None => Err(SliceOobError {
				index,
				length: self.len(),
			}),
		}
	}
}

#[derive(Debug)]
pub struct SliceMutAccessor<'a, T> {
	_ty: PhantomData<&'a ()>,
	root: *mut T,
	len: usize,
}

unsafe impl<'a, T: Send> Send for SliceMutAccessor<'a, T> {}
unsafe impl<'a, T: Sync> Sync for SliceMutAccessor<'a, T> {}

impl<'a, T> From<&'a mut [T]> for SliceMutAccessor<'a, T> {
	fn from(slice: &'a mut [T]) -> Self {
		Self {
			_ty: PhantomData,
			root: slice.as_mut_ptr(),
			len: slice.len(),
		}
	}
}

unsafe impl<'a, T> AssertUnborrowed for SliceMutAccessor<'a, T> {}

unsafe impl<'a, T> Accessor for SliceMutAccessor<'a, T> {
	type Index = usize;
	type Value = T;
	type Ptr = NonNull<T>;
	type OobError = SliceOobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
		if index < self.len {
			Ok(unsafe { NonNull::new_unchecked(self.root.add(index)) })
		} else {
			Err(SliceOobError {
				index,
				length: self.len,
			})
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct SliceOobError {
	pub index: usize,
	pub length: usize,
}

impl Error for SliceOobError {}

impl Display for SliceOobError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"index {} lies out of bounds of the array (array length: {}).",
			self.index, self.length
		)
	}
}

// === Extensions and wrappers === //

pub type PtrOf<T> = <T as Accessor>::Ptr;
pub type RefOf<'a, T> = <PtrOf<T> as PointerLike>::AsRef<'a>;
pub type MutOf<'a, T> = <PtrOf<T> as PointerLike>::AsMut<'a>;

pub trait RefAccessorExt: AssertImmutable {
	fn try_get_ref(&self, index: Self::Index) -> Result<RefOf<'_, Self>, Self::OobError> {
		// Safety: `AssertImmutable` guarantees that all references are logically borrowed for `'_`.
		unsafe { Ok(self.try_get_raw(index)?.promote_ref()) }
	}

	fn get_ref(&self, index: Self::Index) -> RefOf<'_, Self> {
		self.try_get_ref(index).unwrap()
	}
}

impl<T: AssertImmutable> RefAccessorExt for T {}

pub trait MutAccessorExt: AssertUnborrowed {
	fn try_get_mut(&mut self, index: Self::Index) -> Result<MutOf<'_, Self>, Self::OobError> {
		// Safety: `assert_unborrowed_mut` ensures that the entry at `index` is unborrowed before
		// we promote it to a mutable reference.
		unsafe {
			Ok(self
				.assert_unborrowed_mut()
				.try_get_raw(index)?
				.promote_mut())
		}
	}

	fn get_mut(&mut self, index: Self::Index) -> MutOf<'_, Self> {
		self.try_get_mut(index).unwrap()
	}
}

impl<T: AssertUnborrowed> MutAccessorExt for T {}

pub trait OrderedAccessorExt: Accessor
where
	Self::Index: Ord,
{
	fn get_ordered_mut<I>(&mut self, indices: I) -> OrderedAccessorIter<'_, Self, I::IntoIter>
	where
		I: IntoIterator<Item = Self::Index>,
		Self: AssertUnborrowed,
	{
		OrderedAccessorIter {
			accessor: self,
			indices: indices.into_iter(),
			min_index: None,
		}
	}

	// fn split(
	// 	&mut self,
	// 	mid: Self::Index,
	// ) -> (AccessorSplitter<'_, Self>, AccessorSplitter<'_, Self>) {
	// 	(
	// 		AccessorSplitter {
	// 			target: self,
	// 			side: SplitterSide::Left,
	// 			mid,
	// 		},
	// 		AccessorSplitter {
	// 			target: self,
	// 			side: SplitterSide::Right,
	// 			mid,
	// 		},
	// 	)
	// }
}

impl<T> OrderedAccessorExt for T
where
	T: ?Sized + Accessor,
	T::Index: Ord,
{
}

#[derive(Debug)]
pub struct OrderedAccessorIter<'a, A: ?Sized + Accessor, I> {
	accessor: &'a mut A,
	indices: I,
	min_index: Option<A::Index>,
}

impl<'a, A, I> Iterator for OrderedAccessorIter<'a, A, I>
where
	A: Accessor,
	A::Index: Ord,
	I: Iterator<Item = A::Index>,
{
	type Item = <A::Ptr as PointerLike>::AsMut<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		let index = self.indices.next()?;
		let ord = self
			.min_index
			.map_or(Ordering::Less, |min_index| min_index.cmp(&index));
		assert_eq!(
			ord,
			Ordering::Less,
			"Indices in an OrderedAccessorIter must be strictly increasing."
		);
		self.min_index = Some(index);
		Some(unsafe { self.accessor.get_raw(index).promote_mut::<'a>() })
	}
}

// #[derive(Debug)]
// pub struct AccessorSplitter<'a, A: ?Sized + Accessor> {
// 	target: &'a A,
// 	mid: A::Index,
// 	side: SplitterSide,
// }
//
// #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
// pub enum SplitterSide {
// 	Left,
// 	Right,
// }
//
// unsafe impl<'a, A: SemiMutAccessor> SemiMutAccessor for AccessorSplitter<'a, A> where A::Index: Ord {}
// unsafe impl<'a, A: ImmutableAccessor> ImmutableAccessor for AccessorSplitter<'a, A> where
// 	A::Index: Ord
// {
// }
//
// unsafe impl<'a, A> Accessor for AccessorSplitter<'a, A>
// where
// 	A: Accessor,
// 	A::Index: Ord,
// {
// 	type Index = A::Index;
// 	type Value = A::Value;
// 	type Ptr = A::Ptr;
// 	type OobError = SplitterOobError<A>;
//
// 	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
// 		match (self.side, index.cmp(&self.mid)) {
// 			(SplitterSide::Left, Ordering::Less | Ordering::Equal)
// 			| (SplitterSide::Right, Ordering::Greater) => Ok(self
// 				.target
// 				.try_get_raw(index)
// 				.map_err(SplitterOobError::Parent)?),
// 			_ => Err(SplitterOobError::SplitOob {
// 				index,
// 				mid: self.mid,
// 				side: self.side,
// 			}),
// 		}
// 	}
// }
//
// #[derive(Copy, Clone, Hash, Eq, PartialEq)]
// pub enum SplitterOobError<A: Accessor> {
// 	SplitOob {
// 		index: A::Index,
// 		mid: A::Index,
// 		side: SplitterSide,
// 	},
// 	Parent(A::OobError),
// }
//
// impl<A: Accessor> Error for SplitterOobError<A> where A::OobError: Error {}
//
// impl<A: Accessor> Display for SplitterOobError<A>
// where
// 	A::OobError: Display,
// {
// 	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
// 		match self {
// 			SplitterOobError::SplitOob {
// 				index,
// 				mid,
// 				side: SplitterSide::Left,
// 			} => write!(
// 				f,
// 				"index {:?} lies out of bounds of the left splitter range (mid {:?} < index {:?}).",
// 				index, mid, index
// 			),
// 			SplitterOobError::SplitOob {
// 				index,
// 				mid,
// 				side: SplitterSide::Right,
// 			} => write!(
// 				f,
// 				"index {:?} lies out of bounds of the right splitter range (index {:?} <= mid {:?}).",
// 				index, index, mid
// 			),
// 			SplitterOobError::Parent(parent) => Display::fmt(parent, f),
// 		}
// 	}
// }
//
// impl<A: Accessor> Debug for SplitterOobError<A>
// where
// 	A::OobError: Debug,
// {
// 	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
// 		match self {
// 			SplitterOobError::SplitOob { index, mid, side } => f
// 				.debug_struct("SplitterOobError::SplitOob")
// 				.field("index", index)
// 				.field("mid", mid)
// 				.field("side", side)
// 				.finish(),
// 			SplitterOobError::Parent(parent) => f
// 				.debug_tuple("SplitterOobError::Parent")
// 				.field(parent)
// 				.finish(),
// 		}
// 	}
// }

// TODO: Map accessors
