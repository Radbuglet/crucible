use crate::util::error::ResultExt;
use crate::util::iter_ext::ArrayCollectExt;
use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

// === PointerLike === //

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
	type AsRef<'a>: Clone + Deref<Target = Self::Value> where Self: 'a;

	#[rustfmt::skip]
	type AsMut<'a>: Deref<Target = Self::Value> + DerefMut where Self: 'a;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a> where Self: 'a;
	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a> where Self: 'a;
}

#[rustfmt::skip]
impl<T: ?Sized> PointerLike for NonNull<T> {
	type Value = T;

	type AsRef<'a> where Self: 'a = &'a T;
	type AsMut<'a> where Self: 'a = &'a mut T;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a>
	where
		Self: 'a,
	{
		self.as_ref()
	}

	unsafe fn promote_mut<'a>(mut self) -> Self::AsMut<'a>
	where
		Self: 'a,
	{
		self.as_mut()
	}
}

#[rustfmt::skip]
impl<'r, T: ?Sized> PointerLike for &'r UnsafeCell<T> {
	type Value = T;

	type AsRef<'a> where Self: 'a = &'a T;
	type AsMut<'a> where Self: 'a = &'a mut T;

	unsafe fn promote_ref<'a>(self) -> Self::AsRef<'a>
	where
		Self: 'a,
	{
		&*self.get()
	}

	unsafe fn promote_mut<'a>(self) -> Self::AsMut<'a>
	where
		Self: 'a,
	{
		&mut *self.get()
	}
}

// === Promises === //

/// Promises are wrapper objects which allow users to make guarantees about the state of an object
/// which users may break during the duration of their borrow.
///
/// The specific guarantees a `Promise` object makes are determined by the `P` object's documentation.
/// Since promises can be composed, users are recommended to define *traits* in the form of `PromisesFoo`
/// for each promise and document the exact promises made when `P` implements the trait. Users can
/// then list the promises made by an object by setting `P` to *e.g.* `dyn PromisesFoo`. Traits have
/// the benefit of being easily composable by specifying super-traits *e.g.* if `PromisesFoo: PromisesBar`,
/// then having a promise of `P = dyn PromisesFoo` implies having a promise of `dyn PromisesBar` as well.
#[repr(transparent)]
pub struct Promise<T: ?Sized, P: ?Sized> {
	_promise: PhantomData<fn(P) -> P>,
	value: T,
}

impl<T, P: ?Sized> Promise<T, P> {
	pub unsafe fn make(value: T) -> Self {
		Self {
			_promise: PhantomData,
			value,
		}
	}

	pub fn unwrap(self) -> T {
		self.value
	}
}

impl<T: ?Sized, P: ?Sized> Promise<T, P> {
	pub unsafe fn make_ref(value: &T) -> &Self {
		&*(value as *const T as *const Promise<T, P>)
	}

	pub unsafe fn make_mut(value: &mut T) -> &mut Self {
		&mut *(value as *mut T as *mut Promise<T, P>)
	}

	pub fn unwrap_ref(&self) -> &T {
		unsafe { &*(self as *const Promise<T, P> as *const T) }
	}

	pub fn unwrap_mut(&mut self) -> &mut T {
		unsafe { &mut *(self as *mut Promise<T, P> as *mut T) }
	}
}

impl<T: ?Sized, P: ?Sized> Deref for Promise<T, P> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T: ?Sized, P: ?Sized> DerefMut for Promise<T, P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

impl<T: Debug, P: ?Sized> Debug for Promise<T, P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Promise")
			.field(
				"_promise",
				&std::any::type_name::<PhantomData<fn(P) -> P>>(),
			)
			.field("value", &self.value)
			.finish()
	}
}

// === Accessors === //

pub type PtrOf<T> = <T as Accessor>::Ptr;
pub type RefOf<'a, T> = <PtrOf<T> as PointerLike>::AsRef<'a>;
pub type MutOf<'a, T> = <PtrOf<T> as PointerLike>::AsMut<'a>;

/// An `Accessor` represents an object which maps indices to pointers to their contents. A vector
/// mapping indices to elements or a hash map mapping keys to values are examples of an `Accessor`.
/// `Accessors` return [PointerLike] objects, which allows users to unsafely promote the pointer to
/// either a mutable or immutable reference. Wrappers and extension methods can use the guarantees
/// provided by *e.g.* [PromiseUnborrowed], which states that the pointers are mapped injectively and
/// can be promoted mutably, to implement mechanisms to provide mutable references to several distinct
/// values in the `Accessor` at once.
///
/// ## Avoiding Wrapper Chains
///
/// To avoid cascades of `Option<Option<Option<NonNull<T>>>>` when wrapping `Accessors` with out-of-
/// bounds reporting, all accessors have the ability to return an `OobError` type implementing the
/// standard library's [Error] trait. Implementors of this trait are recommended to keep error
/// conditions to the `Err` variant of the result, preferring *e.g.* `Err(FirstWrapperError::Inherit(
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
/// Objects implementing `Deref` where `Target: Accessor` also implement `Accessor`.
///
/// ## Safety
///
/// **No safety guarantees are provided by this trait whatsoever!** By itself, this trait does not
/// guarantee that the returned pointers were mapped injectively or that the pointers are even valid
/// to promote in the first place. These guarantees must be made externally. One common way to make
/// guarantees about objects implementing this trait is through [Promise]s.
pub trait Accessor {
	type Index: Debug + Copy + Eq;
	type Value: ?Sized;

	type Ptr: PointerLike;
	type OobError: Error;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError>;

	fn get_raw(&self, index: Self::Index) -> Self::Ptr {
		self.try_get_raw(index).unwrap_pretty()
	}
}

// Objects implementing `Deref` where `Target: Accessor` also implement `Accessor`.
impl<A: ?Sized + Accessor, T: ?Sized + Deref<Target = A>> Accessor for T {
	type Index = A::Index;
	type Value = A::Value;
	type Ptr = A::Ptr;
	type OobError = A::OobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, A::OobError> {
		(**self).try_get_raw(index)
	}
}

// === Accessor promises === //

pub type PromiseUnborrowed<T> = Promise<T, dyn PromisesUnborrowed>;

pub type PromiseImmutable<T> = Promise<T, dyn PromisesImmutable>;

pub trait AsAccessor<'a> {
	type Accessor: Accessor;

	fn as_accessor(&'a self) -> PromiseImmutable<Self::Accessor>;
}

pub trait AsAccessorMut<'a> {
	type Accessor: Accessor;

	fn as_accessor_mut(&'a mut self) -> PromiseUnborrowed<Self::Accessor>;
}

/// Promises that an [Accessor] behaves injectively and provides the foundations for other more useful
/// promises.
///
/// **Note:** This promise still isn't enough to legally promote returned pointers.
/// See [PromisesImmutable] and [PromisesUnborrowed].
///
/// ## Safety
///
/// When a given `T: Accessor`, `&T`, or `&mut T` makes this promise...
///
/// 1. **Virtual Aliasing:** Accessor entries can be either unborrowed, immutably borrowed, or mutably
///    borrowed. There may only be one exclusive mutable borrow at a given time but there may be an
///    unlimited number of concurrent immutable borrows.
/// 2. **Injectivity:** If two indices are reported to unequal through [Eq], the returned pointers
///    are guaranteed not to logically alias. In other words, this map is injective.
///    TODO: Replace with TrustedEq and TrustedOrd traits.
/// 3. **Promotion Lifetime:** Pointers are valid to promote for at most the lifetime of the **owned**
///    instance `T` so long as virtual aliasing rules are followed. Note that this implicitly means
///    that owning a `T` reference (`&'a T`) will limit promotion lifetimes to the lifetime of that
///    reference (`'a`), not the lifetime of `T`.
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
/// made by external actors such as [PromisesImmutable] and [PromisesUnborrowed].
pub trait PromisesInjective {}

/// Promises that all references returned by an [Accessor] begin in an unborrowed state and can be
/// promoted to immutable references. No guarantees are made about the validity of promoting to a
/// mutable reference (see: [PromisesUnborrowed]).
///
/// ## Safety
///
/// Promises from the safety section of [PromiseInjective] must be kept.
///
/// Upon [unwrap](Promise::unwrap)'ing the [Promise], the returned `T: Accessor` is guaranteed to:
///
/// 1. Start in a logically unborrowed state.
/// 2. Pointers are valid to promote **immutably** so long as the requirements in the "promotion
///    lifetime" section are met (see: [PromiseInjective]). Note that the validity of mutable borrows
///    is not guaranteed by this promise (see: [PromiseUnborrowed]).
///
pub trait PromisesImmutable: PromisesInjective {}

/// Promises that all references returned by an [Accessor] begin in an unborrowed state and can be
/// promoted to immutable or mutable references.
///
/// ## Safety
///
/// Promises from the safety section of [PromiseImmutable] (and, recursively, [PromiseInjective]) must
/// be kept.
///
/// Upon [unwrap](Promise::unwrap)'ing the [Promise], the returned `T: Accessor` is guaranteed to:
///
/// 1. Start in a logically unborrowed state. (inherited from [PromiseImmutable]).
/// 2. Pointers are valid to promote **immutably** so long as the requirements in the "promotion lifetime"
///    section are met (see: [PromiseInjective]). (inherited from [PromiseImmutable])
/// 3. Pointers are valid to promote **mutably** so long as the requirements in the "promotion lifetime"
///    section are met (see: [PromiseInjective]) **and** .
///
pub trait PromisesUnborrowed: PromisesImmutable {}

/// A promise object that strips `S` of all its promises besides [PromisesInjective],
/// [PromisesImmutable], and [PromisesUnborrowed].
pub struct PromisesInheritMutability<S: ?Sized>(PhantomData<S>);

impl<S: ?Sized + PromisesInjective> PromisesInjective for PromisesInheritMutability<S> {}
impl<S: ?Sized + PromisesImmutable> PromisesImmutable for PromisesInheritMutability<S> {}
impl<S: ?Sized + PromisesUnborrowed> PromisesUnborrowed for PromisesInheritMutability<S> {}

// === Core Accessors === //

impl<'a, T: 'a> AsAccessor<'a> for [T] {
	type Accessor = SliceRefAccessor<'a, T>;

	fn as_accessor(&'a self) -> PromiseImmutable<Self::Accessor> {
		unsafe { PromiseImmutable::make(SliceRefAccessor(self)) }
	}
}

impl<'a, T: 'a> AsAccessorMut<'a> for [T] {
	type Accessor = SliceMutAccessor<'a, T>;

	fn as_accessor_mut(&'a mut self) -> PromiseUnborrowed<Self::Accessor> {
		unsafe {
			PromiseUnborrowed::make(SliceMutAccessor {
				_ty: PhantomData,
				root: self.as_mut_ptr(),
				len: self.len(),
			})
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct SliceRefAccessor<'a, T>(pub &'a [T]);

impl<'a, T> Accessor for SliceRefAccessor<'a, T> {
	type Index = usize;
	type Value = T;
	type Ptr = NonNull<T>;
	type OobError = SliceOobError;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
		match self.0.get(index) {
			Some(elem) => Ok(NonNull::from(elem)),
			None => Err(SliceOobError {
				index,
				length: self.0.len(),
			}),
		}
	}
}

#[derive(Debug)]
pub struct SliceMutAccessor<'a, T> {
	_ty: PhantomData<&'a mut [T]>,
	root: *mut T,
	len: usize,
}

unsafe impl<'a, T: Send> Send for SliceMutAccessor<'a, T> {}
unsafe impl<'a, T: Sync> Sync for SliceMutAccessor<'a, T> {}

impl<'a, T> Accessor for SliceMutAccessor<'a, T> {
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

// === Direct access extensions === //

pub trait RefAccessorExt: Accessor {
	fn try_get_ref(&self, index: Self::Index) -> Result<RefOf<'_, Self>, Self::OobError>;

	fn get_ref(&self, index: Self::Index) -> RefOf<'_, Self> {
		self.try_get_ref(index).unwrap_pretty()
	}
}

impl<T: ?Sized + Accessor, P: ?Sized + PromisesImmutable> RefAccessorExt for Promise<T, P> {
	fn try_get_ref(&self, index: Self::Index) -> Result<RefOf<'_, Self>, Self::OobError> {
		unsafe { Ok(self.unwrap_ref().try_get_raw(index)?.promote_ref()) }
	}
}

pub trait MutAccessorExt: Accessor {
	fn try_get_mut(&mut self, index: Self::Index) -> Result<MutOf<'_, Self>, Self::OobError>;

	fn get_mut(&mut self, index: Self::Index) -> MutOf<'_, Self> {
		self.try_get_mut(index).unwrap_pretty()
	}
}

impl<T: ?Sized + Accessor, P: ?Sized + PromisesUnborrowed> MutAccessorExt for Promise<T, P> {
	fn try_get_mut(&mut self, index: Self::Index) -> Result<MutOf<'_, Self>, Self::OobError> {
		unsafe { Ok(self.unwrap_mut().try_get_raw(index)?.promote_mut()) }
	}
}

// === Ordered accessor querying === //

#[derive(Debug)]
pub struct OrderedAccessorIter<'a, A: ?Sized + Accessor, I> {
	accessor: &'a mut A,
	indices: I,
	min_index: Option<A::Index>,
}

impl<'a, A, I> OrderedAccessorIter<'a, A, I>
where
	A: ?Sized + Accessor,
	I: Iterator<Item = A::Index>,
{
	pub fn new<P, TI>(accessor: &'a mut Promise<A, P>, indices: TI) -> Self
	where
		P: ?Sized + PromisesUnborrowed,
		TI: IntoIterator<IntoIter = I>,
	{
		OrderedAccessorIter {
			accessor: accessor.unwrap_mut(),
			indices: indices.into_iter(),
			min_index: None,
		}
	}
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

// === Accessor splitters === //

#[derive(Debug)]
pub struct AccessorSplitter<'a, A: ?Sized + Accessor> {
	target: &'a A,
	mid: A::Index,
	side: SplitterSide,
}

impl<'a, A: Accessor> AccessorSplitter<'a, A> {
	pub fn new<P: ?Sized>(
		accessor: &'a mut Promise<A, P>,
		mid: A::Index,
	) -> (
		Promise<Self, PromisesInheritMutability<P>>,
		Promise<Self, PromisesInheritMutability<P>>,
	) {
		unsafe {
			(
				Promise::make(Self {
					target: accessor.unwrap_ref(),
					mid,
					side: SplitterSide::Left,
				}),
				Promise::make(Self {
					target: accessor.unwrap_ref(),
					mid,
					side: SplitterSide::Right,
				}),
			)
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum SplitterSide {
	Left,
	Right,
}

impl<'a, A> Accessor for AccessorSplitter<'a, A>
where
	A: Accessor,
	A::Index: Ord,
{
	type Index = A::Index;
	type Value = A::Value;
	type Ptr = A::Ptr;
	type OobError = SplitterOobError<A>;

	fn try_get_raw(&self, index: Self::Index) -> Result<Self::Ptr, Self::OobError> {
		match (self.side, index.cmp(&self.mid)) {
			(SplitterSide::Left, Ordering::Less | Ordering::Equal)
			| (SplitterSide::Right, Ordering::Greater) => Ok(self
				.target
				.try_get_raw(index)
				.map_err(SplitterOobError::Parent)?),
			_ => Err(SplitterOobError::SplitOob {
				index,
				mid: self.mid,
				side: self.side,
			}),
		}
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub enum SplitterOobError<A: Accessor> {
	SplitOob {
		index: A::Index,
		mid: A::Index,
		side: SplitterSide,
	},
	Parent(A::OobError),
}

impl<A: Accessor> Error for SplitterOobError<A> where A::OobError: Error {}

impl<A: Accessor> Display for SplitterOobError<A>
where
	A::OobError: Display,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			SplitterOobError::SplitOob {
				index,
				mid,
				side: SplitterSide::Left,
			} => write!(
				f,
				"index {:?} lies out of bounds of the left splitter range (mid {:?} < index {:?}).",
				index, mid, index
			),
			SplitterOobError::SplitOob {
				index,
				mid,
				side: SplitterSide::Right,
			} => write!(
				f,
				"index {:?} lies out of bounds of the right splitter range (index {:?} <= mid {:?}).",
				index, index, mid
			),
			SplitterOobError::Parent(parent) => Display::fmt(parent, f),
		}
	}
}

impl<A: Accessor> Debug for SplitterOobError<A>
where
	A::OobError: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			SplitterOobError::SplitOob { index, mid, side } => f
				.debug_struct("SplitterOobError::SplitOob")
				.field("index", index)
				.field("mid", mid)
				.field("side", side)
				.finish(),
			SplitterOobError::Parent(parent) => f
				.debug_tuple("SplitterOobError::Parent")
				.field(parent)
				.finish(),
		}
	}
}

// === Tests === //

#[test]
fn accessor_splitter_test() {
	let mut my_vec = vec![2, 3, 4, 5, 6];
	let mut accessor = my_vec.as_accessor_mut();
	*accessor.get_mut(2) = 2;

	let (mut left, right) = AccessorSplitter::new(&mut accessor, 2);
	let [a, b, c] = OrderedAccessorIter::new(&mut left, [0, 1, 2]).collect_array();
	*a = 1;
	*b = 2;
	*c = 3;

	assert_eq!(*right.get_ref(3), 5);
	assert_eq!(*right.get_ref(4), 6);
	assert!(left.try_get_ref(4).is_err());
	assert!(right.try_get_ref(2).is_err());
	assert_eq!(my_vec, [1, 2, 3, 5, 6]);
}
