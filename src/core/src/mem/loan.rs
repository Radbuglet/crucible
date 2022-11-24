use core::{
	mem::ManuallyDrop,
	ops::{Deref, DerefMut},
};

// === Core === //

pub trait Lender: Sized {
	type Loan;
	type Shark;

	fn loan(me: Self) -> (Self::Loan, Self::Shark);

	fn repay(loan: Self::Loan, shark: Self::Shark);

	fn map<U, F>(self, f: F) -> Mapped<Self, U>
	where
		F: FnOnce(Self::Loan) -> U,
		U: Borrower<Self::Loan>,
	{
		let (loan, shark) = Self::loan(self);
		let borrower = f(loan);

		Mapped {
			shark: ManuallyDrop::new(shark),
			borrower: ManuallyDrop::new(borrower),
		}
	}
}

pub trait Borrower<L> {
	fn drop_and_repay(self) -> L;
}

pub struct Mapped<A: Lender, B: Borrower<A::Loan>> {
	shark: ManuallyDrop<A::Shark>,
	borrower: ManuallyDrop<B>,
}

impl<A, B> Deref for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	type Target = B;

	fn deref(&self) -> &Self::Target {
		&self.borrower
	}
}

impl<A, B> DerefMut for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.borrower
	}
}

impl<A, B> Drop for Mapped<A, B>
where
	A: Lender,
	B: Borrower<A::Loan>,
{
	fn drop(&mut self) {
		let borrower = unsafe { ManuallyDrop::take(&mut self.borrower) };
		let loan = borrower.drop_and_repay();

		let shark = unsafe { ManuallyDrop::take(&mut self.shark) };
		A::repay(loan, shark);
	}
}

// === Primitives === //

// TODO
