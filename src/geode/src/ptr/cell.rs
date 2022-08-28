use crucible_core::{cell::UnsafeCellExt, sync::MutexedUnsafeCell};

use crate::core::{
	lock::{BorrowMutability, Lock, UserLock},
	session::Session,
};

pub struct LUseCell<T: ?Sized> {
	lock: MutexedUnsafeCell<Lock>,
	value: MutexedUnsafeCell<T>,
}

impl<T: ?Sized> LUseCell<T> {
	pub fn new(lock: UserLock, value: T) -> Self
	where
		T: Sized,
	{
		Self {
			lock: MutexedUnsafeCell::new(lock.as_lock()),
			value: MutexedUnsafeCell::new(value),
		}
	}

	fn failed_to_borrow(_lock: Lock, _session: Session) -> ! {
		// TODO: Implement better error reporting
		panic!("Failed to borrow LUseCell.");
	}

	pub fn update_ref<F, R>(&self, session: Session, f: F) -> R
	where
		F: FnOnce(&T) -> R,
	{
		let lock = unsafe { *self.lock.get_ref_unchecked() };

		// TODO: Could we reduce the number of branches needed by this routine?
		match lock.session_borrow_state(session) {
			Some(BorrowMutability::Mut) => {
				// Prevent further mutable access to lock.
				unsafe { *self.lock.get_mut_unchecked() = Lock::ALWAYS_IMMUTABLE };

				// Run critical section
				let ret = f(unsafe { self.value.get_ref_unchecked() });

				// Return lock to previous state.
				unsafe { *self.lock.get_mut_unchecked() = lock };

				ret
			}
			Some(BorrowMutability::Ref) => {
				// Users cannot lock this cell mutably so just run the critical section without any
				// additional considerations.
				f(unsafe { self.value.get_ref_unchecked() })
			}
			None => Self::failed_to_borrow(lock, session),
		}
	}

	pub fn update_mut<F, R>(&self, session: Session, f: F) -> R
	where
		F: FnOnce(&mut T) -> R,
	{
		let lock = unsafe { *self.lock.get_mut_unchecked() };

		if lock.is_borrowed_mutably_by(session) {
			// Prevent further access to lock.
			unsafe { *self.lock.get_mut_unchecked() = Lock::ALWAYS_UNBORROWED };

			// Run critical section
			let ret = f(unsafe { self.value.get_mut_unchecked() });

			// Return lock to previous state.
			unsafe { *self.lock.get_mut_unchecked() = lock };

			ret
		} else {
			Self::failed_to_borrow(lock, session)
		}
	}
}
