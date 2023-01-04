use std::{
	fmt, hash,
	ops::{Deref, DerefMut},
	sync::Arc,
};

use crate::{
	debug::userdata::{ErasedUserdata, Userdata},
	mem::{
		drop_guard::{DropOwned, DropOwnedGuard},
		ptr::addr_of_ptr,
	},
};

// === Lender and Borrower === //

pub trait Lender: Sized {
	type Loan;
	type Shark;

	fn loan(me: Self) -> (Self::Loan, Self::Shark);

	unsafe fn repay(loan: Self::Loan, shark: Self::Shark) -> Self;
}

pub unsafe trait Borrower<L> {
	fn drop_and_repay(self) -> L;
}

// === Mapped === //

pub struct Mapped<A: Lender, B: Borrower<A::Loan>>(DropOwnedGuard<MappedInner<A, B>>);

impl<A, B> Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	pub unsafe fn new(shark: A::Shark, borrower: B) -> Self {
		Self(MappedInner { shark, borrower }.into())
	}

	pub fn unwrap(me: Self) -> A {
		DropOwnedGuard::defuse(me.0).unwrap()
	}
}

impl<A, B> Lender for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + Lender,
{
	type Loan = B::Loan;
	type Shark = (B::Shark, A::Shark);

	fn loan(me: Self) -> (Self::Loan, Self::Shark) {
		let inner = DropOwnedGuard::defuse(me.0);

		let a_shark = inner.shark;
		let (b_loan, b_shark) = B::loan(inner.borrower);

		(b_loan, (b_shark, a_shark))
	}

	unsafe fn repay(b_loan: Self::Loan, (b_shark, a_shark): Self::Shark) -> Self {
		let borrowed = B::repay(b_loan, b_shark);

		Self::new(a_shark, borrowed)
	}
}

impl<A, B> Deref for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	type Target = B;

	fn deref(&self) -> &Self::Target {
		&self.0.borrower
	}
}

impl<A, B> DerefMut for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0.borrower
	}
}

impl<A, B> fmt::Debug for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("LentRef").field(&&*self).finish()
	}
}

impl<A, B> Eq for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + Eq,
{
}

impl<A, B> PartialEq for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		&**self == &**other
	}
}

impl<A, B> Ord for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + Ord,
{
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(**self).cmp(&**other)
	}
}

impl<A, B> PartialOrd for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		(**self).partial_cmp(&**other)
	}
}

impl<A, B> hash::Hash for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan> + hash::Hash,
{
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

struct MappedInner<A: Lender, B: Borrower<A::Loan>> {
	shark: A::Shark,
	borrower: B,
}

impl<A, B> MappedInner<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	pub fn unwrap(self) -> A {
		let loan = self.borrower.drop_and_repay();
		unsafe {
			// Safety: the `Borrower` trait `impl` guarantees that it will return the same loan it
			// was given when we constructed the loan.
			A::repay(loan, self.shark)
		}
	}
}

impl<A: Lender, B: Borrower<A::Loan>> DropOwned for MappedInner<A, B> {
	fn drop_owned(self, _cx: ()) {
		drop(self.unwrap());
	}
}

// === LentRef === //

pub struct LentRef<T: ?Sized>(*const T);

impl<T: ?Sized> LentRef<T> {
	pub unsafe fn new(r: *const T) -> Self {
		Self(r)
	}

	pub fn new_safe(r: &'static T) -> Self {
		Self(r)
	}

	pub fn as_ptr(me: &Self) -> *const T {
		me.0
	}

	pub fn to_ptr(me: Self) -> *const T {
		me.0
	}
}

impl<T: ?Sized> Deref for LentRef<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.0 }
	}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for LentRef<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("LentRef").field(&&*self).finish()
	}
}

impl<T: ?Sized + Eq> Eq for LentRef<T> {}

impl<T: ?Sized + PartialEq> PartialEq for LentRef<T> {
	fn eq(&self, other: &Self) -> bool {
		&**self == &**other
	}
}

impl<T: ?Sized + Ord> Ord for LentRef<T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(**self).cmp(&**other)
	}
}

impl<T: ?Sized + PartialOrd> PartialOrd for LentRef<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		(**self).partial_cmp(&**other)
	}
}

impl<T: ?Sized + hash::Hash> hash::Hash for LentRef<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

// === LentMut === //

pub struct LentMut<T: ?Sized>(*mut T);

impl<T: ?Sized> LentMut<T> {
	pub unsafe fn new(r: *mut T) -> Self {
		Self(r)
	}

	pub fn new_safe(r: &'static mut T) -> Self {
		Self(r)
	}

	pub fn as_ptr(me: &Self) -> *const T {
		me.0
	}

	pub fn as_ptr_mut(me: &mut Self) -> *mut T {
		me.0
	}

	pub fn to_ptr(me: Self) -> *mut T {
		me.0
	}
}

impl<T: ?Sized> Deref for LentMut<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.0 }
	}
}

impl<T: ?Sized> DerefMut for LentMut<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.0 }
	}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for LentMut<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("LentMut").field(&&*self).finish()
	}
}

impl<T: ?Sized + Eq> Eq for LentMut<T> {}

impl<T: ?Sized + PartialEq> PartialEq for LentMut<T> {
	fn eq(&self, other: &Self) -> bool {
		&**self == &**other
	}
}

impl<T: ?Sized + Ord> Ord for LentMut<T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(**self).cmp(&**other)
	}
}

impl<T: ?Sized + PartialOrd> PartialOrd for LentMut<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		(**self).partial_cmp(&**other)
	}
}

impl<T: ?Sized + hash::Hash> hash::Hash for LentMut<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(&**self).hash(state);
	}
}

// === Box Mapping === //

pub unsafe fn map_box<T: ?Sized, U: ?Sized, F>(b: Box<T>, f: F) -> Box<U>
where
	F: FnOnce(&mut T) -> &mut U,
{
	let original = Box::leak(b);
	let original_ptr = original as *mut T;
	let converted = f(original);
	assert_eq!(addr_of_ptr(original_ptr), addr_of_ptr(converted));

	// Safety: `f` gives a proof that it can convert a reference of `&'a mut T` into a reference of
	// `&'a mut U` lasting for an arbitrary caller-selected lifetime. Additionally, because the
	// pointer address is the same, we know that we're pointing to a valid `Box`.
	//
	// TODO: Document last necessary safety guarantee once drop rules are clarified.
	Box::from_raw(converted)
}

pub fn downcast_userdata_box<T: Userdata>(b: Box<dyn Userdata>) -> Box<T> {
	unsafe { map_box(b, |val| val.downcast_mut::<T>()) }
}

pub unsafe fn map_arc<T: ?Sized, U: ?Sized, F>(arc: Arc<T>, f: F) -> Arc<U>
where
	F: FnOnce(&T) -> &U,
{
	let ptr = Arc::into_raw(arc);
	let converted = f(unsafe { &*ptr }) as *const U;
	assert_eq!(addr_of_ptr(ptr), addr_of_ptr(converted));

	// Safety: `f` gives a proof that it can convert a reference of `&'a T` into a reference of
	// `&'a U`. Additionally, because the pointer address is the same, we know that we're pointing
	// to a valid `Arc`.
	//
	// TODO: Document last necessary safety guarantee once drop rules are clarified.
	Arc::from_raw(converted)
}

pub fn downcast_userdata_arc<T: Userdata>(arc: Arc<dyn Userdata>) -> Arc<T> {
	unsafe { map_arc(arc, |val| val.downcast_ref::<T>()) }
}
