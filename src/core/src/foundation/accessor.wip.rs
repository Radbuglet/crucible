use std::cmp::Ordering;
use std::fmt::Debug;
use std::ptr::NonNull;

// === Core traits === //

/// An `Accessor` represents an object which maps indices to distinct values in a one-to-one fashion.
/// A vector mapping indices to elements or a hash map mapping keys to values are examples of an
/// `Accessor`. `Accessors` return [AnyRef] references, a type of reference which can be unsafely
/// promoted to either a mutable or immutable reference. Wrappers and extension methods can use the
/// one-to-one property of the map alongside [AnyRef] promotion to implement mechanisms to provide
/// mutable references to several distinct values in the [Accessor] at once.
///
/// ## Safety
///
/// TODO
///
pub unsafe trait Accessor {
	type Index: Debug + Copy + Eq;
	type Ptr: PointerLike;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr;
}

/// A raw reference type that can be promoted into either its mutable or immutable form.
#[rustfmt::skip]
pub trait PointerLike {
	type AsRef<'a> where Self: 'a;
	type AsMut<'a> where Self: 'a;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a>;
	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a>;
}

// === Core AccessedElem impls === //

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

unsafe impl<T> Accessor for [T] {
	type Index = usize;
	type Ptr = Option<NonNull<T>>;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		self.get(index).map(|ref_| NonNull::from(ref_))
	}
}

// === Extensions and wrappers === //

pub trait SingleBorrowAccessorExt: Accessor {
	fn get_ref(&self, index: Self::Index) -> <Self::Ptr as PointerLike>::AsRef<'_> {
		unsafe { self.get_raw(index).promote_ref() }
	}

	fn get_mut(&mut self, index: Self::Index) -> <Self::Ptr as PointerLike>::AsMut<'_> {
		unsafe { self.get_raw(index).promote_mut() }
	}
}

impl<T: ?Sized + Accessor> SingleBorrowAccessorExt for T {}

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

// FIXME: This impl isn't actually safe because we don't have mut access to `target`.
unsafe impl<'a, A> Accessor for AccessorSplitter<'a, A>
where
	A: Accessor,
	A::Index: Ord,
{
	type Index = A::Index;
	type Ptr = A::Ptr;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		// Validate index
		match (self.is_right, index.cmp(&self.mid)) {
			(false, Ordering::Less | Ordering::Equal) => {}
			(true, Ordering::Greater) => {}
			_ => match self.is_right {
				false => panic!(
					"Out of bounds access on left splitter. ({:?} > {:?})",
					index, self.mid
				),
				true => panic!(
					"Out of bounds access on right splitter. ({:?} <= {:?})",
					index, self.mid
				),
			},
		}

		// Produce pointer
		self.target.get_raw(index)
	}
}

#[test]
fn foo() {
	let mut foo = vec![1, 4, 5, 3, 2];
	println!(
		"{:?}",
		(&mut *foo).get_ordered_mut([1, 2, 3]).collect::<Vec<_>>()
	);
}
