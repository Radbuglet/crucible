use core::fmt;
use std::{
	hash,
	ops::{Deref, DerefMut},
	sync::Arc,
};

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::userdata::{ErasedUserdataValue, UserdataValue},
	mem::{
		drop_guard::{DropGuard, DropGuardHandler},
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

pub struct Mapped<A: Lender, B: Borrower<A::Loan>>(DropGuard<MappedInner<A, B>, MappedDropHandler>);

impl<A, B> Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	pub unsafe fn new(shark: A::Shark, borrower: B) -> Self {
		Self(DropGuard::new(
			MappedInner { shark, borrower },
			MappedDropHandler,
		))
	}

	pub fn unwrap(me: Self) -> A {
		DropGuard::defuse(me.0).unwrap()
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
		let inner = DropGuard::defuse(me.0);

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

struct MappedDropHandler;

impl<A, B> DropGuardHandler<MappedInner<A, B>> for MappedDropHandler
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	fn destruct(self, inner: MappedInner<A, B>) {
		drop(inner.unwrap());
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

// === Arc Lender === //

impl<T: ?Sized + 'static> Lender for Arc<T> {
	type Loan = LentRef<T>;
	type Shark = ();

	fn loan(me: Self) -> (Self::Loan, Self::Shark) {
		let loan = Arc::into_raw(me);
		let loan = unsafe { LentRef::new(loan) };

		(loan, ())
	}

	unsafe fn repay(loan: Self::Loan, _shark: Self::Shark) -> Self {
		let ptr = LentRef::to_ptr(loan);
		unsafe { Arc::from_raw(ptr) }
	}
}

// === BorrowingRwReadGuard === //

#[derive(Debug)]
pub struct BorrowingRwReadGuard<T: ?Sized + 'static>(RwLockReadGuard<'static, T>);

impl<T: ?Sized + 'static> BorrowingRwReadGuard<T> {
	pub fn try_new<L>(lender: L) -> Result<Mapped<L, Self>, L>
	where
		L: Lender<Loan = LentRef<RwLock<T>>>,
	{
		let (loan, shark) = L::loan(lender);
		let lock = unsafe { &*LentRef::as_ptr(&loan) };

		let Some(guard) = lock.try_read() else {
			return Err(unsafe { L::repay(loan, shark) });
		};
		drop(loan); // `Loan` converted into a `guard`.

		Ok(unsafe { Mapped::new(shark, Self(guard)) })
	}
}

impl<T: ?Sized + 'static> Deref for BorrowingRwReadGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

unsafe impl<T: ?Sized + 'static> Borrower<LentRef<RwLock<T>>> for BorrowingRwReadGuard<T> {
	fn drop_and_repay(self) -> LentRef<RwLock<T>> {
		let loan = RwLockReadGuard::rwlock(&self.0) as *const _;
		drop(self);

		unsafe { LentRef::new(loan) }
	}
}

// === BorrowingRwWriteGuard === //

#[derive(Debug)]
pub struct BorrowingRwWriteGuard<T: ?Sized + 'static>(RwLockWriteGuard<'static, T>);

impl<T: ?Sized + 'static> BorrowingRwWriteGuard<T> {
	pub fn try_new<L>(lender: L) -> Result<Mapped<L, Self>, L>
	where
		L: Lender<Loan = LentRef<RwLock<T>>>,
	{
		let (loan, shark) = L::loan(lender);
		let lock = unsafe { &*LentRef::as_ptr(&loan) };

		let Some(guard) = lock.try_write() else {
			return Err(unsafe { L::repay(loan, shark) });
		};
		drop(loan); // `Loan` converted into a `guard`.

		Ok(unsafe { Mapped::new(shark, Self(guard)) })
	}
}

impl<T: ?Sized + 'static> Deref for BorrowingRwWriteGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized + 'static> DerefMut for BorrowingRwWriteGuard<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

unsafe impl<T: ?Sized + 'static> Borrower<LentRef<RwLock<T>>> for BorrowingRwWriteGuard<T> {
	fn drop_and_repay(self) -> LentRef<RwLock<T>> {
		let loan = RwLockWriteGuard::rwlock(&self.0) as *const _;
		drop(self);

		unsafe { LentRef::new(loan) }
	}
}

// === Arc<T> => Arc<U> === //

pub fn map_arc<T: ?Sized, U: ?Sized, F>(arc: Arc<T>, f: F) -> Arc<U>
where
	F: FnOnce(&T) -> &U,
{
	let ptr = Arc::into_raw(arc);
	let converted = f(unsafe { &*ptr }) as *const U;
	assert_eq!(addr_of_ptr(ptr), addr_of_ptr(converted));

	unsafe {
		// Safety: `f` gives a proof that it can convert a reference of `&'a T` into a reference of
		// `&'a U`. Additionally, because the pointer address is the same, we know that we're pointing
		// to a valid `Arc`.
		Arc::from_raw(converted)
	}
}

pub fn downcast_userdata_arc<T: UserdataValue>(arc: Arc<dyn UserdataValue>) -> Arc<T> {
	map_arc(arc, |val| val.downcast_ref::<T>())
}
