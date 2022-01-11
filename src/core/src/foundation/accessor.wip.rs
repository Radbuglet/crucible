use std::cmp::Ordering;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ptr::NonNull;

// === Core traits === //

/// Allows users to convert the object into an [Accessor].
///
/// ## Derived Implementations
///
/// An implementation of [IntoAccessor] on `&T` will imply an implementation on `&mut T` and an
/// implementation on `&mut T` will imply an implementation on `T`.
///
/// ## Safety
///
/// The implementor must guarantee that all elements in the returned accessor begin in an unborrowed
/// state.
///
pub unsafe trait IntoAccessor {
	type Accessor: Accessor;

	fn to_accessor(self) -> Self::Accessor;
}

/// An `Accessor` represents an object which maps indices to distinct values in a one-to-one fashion.
/// A vector mapping indices to elements or a hash map mapping keys to values are examples of an
/// `Accessor`. `Accessors` return [AnyRef] references, a type of reference which can be unsafely
/// promoted to either a mutable or immutable reference. Wrappers and extension methods can use the
/// one-to-one property of the map alongside [AnyRef] promotion to implement mechanisms to provide
/// mutable references to several distinct values in the [Accessor] at once.
///
/// ## Derived Implementations
///
/// Both mutable and immutable references to [Accessor]s also implement [Accessor].
///
/// ## Safety
///
/// 1. If two indices are reported to be equal through [Eq], the returned pointers are guaranteed not
///    to alias. TODO: Replace with TrustedEq and TrustedOrd traits.
/// 2. So long as the virtual aliasing rules described by this safety section are followed properly,
///    returned pointers will be valid to promote for at most the lifetime of the accessor object.
///    Note that because `Accessors` take immutable references to themselves, promoting to a mutable
///    reference may cause undefined behavior unless the target exhibits proper interior mutability
///    with [UnsafeCell](std::cell::UnsafeCell) (or its derivatives) or stores a mutable raw pointer
///    to the underlying target where it does not conflict with any existing immutable references.
///
/// Note that plain accessors are *not* guaranteed to be entirely unborrowed and some subset of any
/// given accessor may have already been borrowed by some external actor. Additional guarantees may
/// be provided by external contracts such as those provided by [ToAccessor].
///
/// TODO: Preserve iterators; how do we make OOB pointers more ergonomic?
pub unsafe trait Accessor {
	type Index: Debug + Copy + Eq;
	type Ptr: PointerLike;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr;
}

unsafe impl<'a, T: Accessor> Accessor for &'a T {
	type Index = T::Index;
	type Ptr = T::Ptr;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		*self.get_raw(index)
	}
}

unsafe impl<'a, T: Accessor> Accessor for &'a mut T {
	type Index = T::Index;
	type Ptr = T::Ptr;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		*self.get_raw(index)
	}
}

/// A raw reference type that can be promoted into either its mutable or immutable form.
///
/// ## Safety
///
/// [PointerLike]s carry no safety guarantees about promotion validity by themselves, and their
/// semantics must typically be augmented by some external contract. However, when a contract specifies
/// that "promotion is legal" with a specified lifetime, the produced reference must be safe to use,
/// even in safe contexts. This means that returned reference lifetimes must be properly bounded.
#[rustfmt::skip]
pub trait PointerLike {
	// `Self: 'a` provides a concise (albeit overly-conservative) way of ensuring that the pointee
	// lives as long as the lifetime since objects can only live as long as the lifetimes of their
	// generic parameters.
	type AsRef<'a> where Self: 'a;
	type AsMut<'a> where Self: 'a;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a>;
	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a>;
}

// === Core PointerLike impls === //

#[rustfmt::skip]
impl<T: PointerLike> PointerLike for Option<T> {
	type AsRef<'a> where Self: 'a = Option<T::AsRef<'a>>;
	type AsMut<'a> where Self: 'a = Option<T::AsMut<'a>>;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a> {
		self.map(|inner| inner.promote_ref())
	}

	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a> {
		self.map(|inner| inner.promote_mut())
	}
}

#[rustfmt::skip]
impl<T: ?Sized> PointerLike for NonNull<T> {
	type AsRef<'a> where Self: 'a = &'a T;
	type AsMut<'a> where Self: 'a = &'a mut T;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a> {
		self.as_ref()
	}

	unsafe fn promote_mut<'a>(mut self) -> Self::AsMut<'a> {
		self.as_mut()
	}
}

// === Core Accessor impls === //

#[derive(Debug)]
pub struct SliceAccessor<'a, T> {
	_ty: PhantomData<&'a ()>,
	root: *mut T,
	len: usize,
}

unsafe impl<'a, T: Send> Send for SliceAccessor<'a, T> {}
unsafe impl<'a, T: Sync> Sync for SliceAccessor<'a, T> {}

unsafe impl<'a, T> IntoAccessor for &'a mut [T] {
	type Accessor = SliceAccessor<'a, T>;

	fn to_accessor(self) -> Self::Accessor {
		Self {
			_ty: PhantomData,
			root: self.as_mut_ptr(),
			len: self.len(),
		}
	}
}

unsafe impl<'a, T> Accessor for SliceAccessor<'a, T> {
	type Index = usize;
	type Ptr = Option<NonNull<T>>;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		if index < self.len {
			Some(unsafe { NonNull::new_unchecked(self.root.add(index)) })
		} else {
			None
		}
	}
}

/*
// === Extensions and wrappers === //

pub trait SingleBorrowAccessorExt: IntoAccessor {
	fn get_ref(self, index: Self::Index) -> <Self::Ptr as PointerLike>::AsRef<'_> {
		unsafe {
			// Safety: newly created accessor are guaranteed to be entirely unborrowed.
			self.to_accessor().get_raw(index).promote_ref()
		}
	}

	fn get_mut(self, index: Self::Index) -> <Self::Ptr as PointerLike>::AsMut<'_> {
		unsafe {
			// Safety: newly created accessor are guaranteed to be entirely unborrowed.
			self.to_accessor().get_raw(index).promote_mut()
		}
	}
}

impl<T: IntoAccessor> SingleBorrowAccessorExt for T {}

pub trait OrderedAccessorExt: Accessor
where
	Self::Index: Ord,
{
	fn get_ordered_mut<I>(&mut self, indices: I) -> OrderedAccessorIter<'_, Self, I::IntoIter>
	where
		I: IntoIterator<Item = Self::Index>,
	{
		OrderedAccessorIter {
			accessor: self,
			indices: indices.into_iter(),
			min_index: None,
		}
	}

	fn split(
		&mut self,
		mid: Self::Index,
	) -> (AccessorSplitter<'_, Self>, AccessorSplitter<'_, Self>) {
		(
			AccessorSplitter {
				target: self,
				is_right: false,
				mid,
			},
			AccessorSplitter {
				target: self,
				is_right: true,
				mid,
			},
		)
	}
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

#[derive(Debug)]
pub struct AccessorSplitter<'a, A: ?Sized + Accessor> {
	target: &'a A,
	mid: A::Index,
	is_right: bool,
}

unsafe impl<'a, A> Accessor for AccessorSplitter<'a, A>
where
	A: Accessor,
	A::Index: Ord,
{
	type Index = A::Index;
	type Ptr = Option<A::Ptr>;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		match (self.is_right, index.cmp(&self.mid)) {
			(false, Ordering::Less | Ordering::Equal) | (true, Ordering::Greater) => {
				Some(self.target.get_raw(index))
			}
			_ => None,
		}
	}
}
*/
