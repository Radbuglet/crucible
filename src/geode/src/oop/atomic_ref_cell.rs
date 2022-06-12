use crate::util::error::ResultExt;
use crate::util::usually::UsuallySafeCell;
use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::Unsize;
use std::ops::{CoerceUnsized, Deref, DerefMut};
use std::sync::atomic::{AtomicIsize, Ordering as AtomicOrdering};

// === Locking === //

const TOO_MANY_REFS_ERROR: &str =
	"Cannot create more than isize::MAX concurrent references to an `ARefCell`.";

#[derive(Default)]
struct LockCounter {
	// Interpreting the values:
	// - Positive values mean the lock is mutably locked (there can be more than one active mutable
	// borrow in the case of split borrows).
	// - Zero means the lock is unborrowed.
	// - Negative means the lock is immutably locked.
	//
	// This `AtomicIsize` is wrapped in a `UsuallySafeCell` to allow unsafe unsynchronized calls to
	// promote their `&self` to a `&mut AtomicIsize`. This should allow `ARefCell` to become a drop-in
	// replacement for `RefCell` without any performance penalty. Unfortunately, we haven't
	// implemented this for `Obj` yet.
	rc: UsuallySafeCell<AtomicIsize>,
}

impl Debug for LockCounter {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LockCounter")
			.field("rc", &self.snapshot())
			.finish()
	}
}

impl LockCounter {
	pub fn snapshot(&self) -> LockState {
		LockState::parse(self.rc.load(AtomicOrdering::Relaxed))
	}

	pub fn undo_leak(&mut self) {
		*self.rc.get_mut() = 0;
	}
}

type WriteGuard<'a> = LockGuard<'a, true>;
type ReadGuard<'a> = LockGuard<'a, false>;

struct LockGuard<'a, const IS_WRITE: bool> {
	target: &'a LockCounter,
}

impl<'a, const IS_WRITE: bool> LockGuard<'a, IS_WRITE> {
	/// Attempts to create a new lock over the provided [LockCounter], returning a [LockError] if it
	/// fails. This is semantically equivalent to locking the entire container, which means that:
	///
	/// - mutably borrowed containers cannot be immutably borrowed and vice-versa.
	/// - mutably borrowed containers cannot be mutably borrowed again, even though multiple concurrent
	///   mutable borrows are sometimes possible in the case of guard splitting. To handle cases such
	///   as guard splitting, `clone` the lock guard instead.
	///
	pub fn try_lock(target: &'a LockCounter) -> Result<Self, LockError> {
		let result = target.rc.fetch_update(
			AtomicOrdering::Acquire, // FIXME: Why is *this* the ordering `rustc` suggests to me?!
			AtomicOrdering::Relaxed,
			|val| match IS_WRITE {
				// Run write behavior
				true => {
					if val == 0 {
						Some(1)
					} else {
						None
					}
				}
				// Run read behavior
				false => {
					if val <= 0 {
						Some(val.checked_sub(1).expect(TOO_MANY_REFS_ERROR))
					} else {
						None
					}
				}
			},
		);

		match result {
			Ok(_) => Ok(Self { target }),
			Err(state) => Err(LockError {
				state: LockState::parse(state),
			}),
		}
	}

	pub unsafe fn try_lock_unsynchronized(target: &'a LockCounter) -> Result<Self, LockError> {
		let rc = target.rc.unchecked_get_mut().get_mut();
		let result = match IS_WRITE {
			// Run write behavior
			true => {
				if *rc == 0 {
					*rc = 1;
					true
				} else {
					false
				}
			}
			// Run read behavior
			false => {
				if *rc <= 0 {
					*rc = rc.checked_sub(1).expect(TOO_MANY_REFS_ERROR);
					true
				} else {
					false
				}
			}
		};

		match result {
			true => Ok(Self { target }),
			false => Err(LockError {
				state: LockState::parse(*rc),
			}),
		}
	}

	const fn acquire_delta() -> isize {
		if IS_WRITE {
			1
		} else {
			-1
		}
	}
}

impl<const IS_WRITE: bool> Clone for LockGuard<'_, IS_WRITE> {
	fn clone(&self) -> Self {
		let success = self
			.target
			.rc
			.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |val| {
				val.checked_add(Self::acquire_delta())
			})
			.is_ok();

		assert!(success, "{TOO_MANY_REFS_ERROR}");

		Self {
			target: self.target,
		}
	}
}

impl<const IS_WRITE: bool> Drop for LockGuard<'_, IS_WRITE> {
	fn drop(&mut self) {
		self.target
			.rc
			.fetch_sub(Self::acquire_delta(), AtomicOrdering::Release);
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum LockState {
	Mutably(usize),
	Immutably(usize),
	Unborrowed,
}

impl LockState {
	fn parse(value: isize) -> Self {
		match 0.cmp(&value) {
			Ordering::Less => LockState::Immutably(-value as usize),
			Ordering::Equal => LockState::Unborrowed,
			Ordering::Greater => LockState::Mutably(value as usize),
		}
	}
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LockError {
	pub state: LockState,
}

impl Error for LockError {}

impl Display for LockError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "failed to lock value ")?;
		match self.state {
			LockState::Mutably(concurrent) => {
				write!(
					f,
					"immutably: {} concurrent mutable borrow{} prevent{} shared immutable access",
					concurrent,
					// Gotta love English grammar
					if concurrent == 1 { "" } else { "s" },
					if concurrent == 1 { "s" } else { "" },
				)?;
			}
			LockState::Immutably(concurrent) => {
				write!(
					f,
					"mutably: {} concurrent immutable borrow{} prevent{} exclusive mutable access",
					concurrent,
					// Gotta love English grammar
					if concurrent == 1 { "" } else { "s" },
					if concurrent == 1 { "s" } else { "" },
				)?;
			}
			LockState::Unborrowed => {
				f.write_str("even though it was unborrowed?!")?;
			}
		}
		Ok(())
	}
}

// === ARefCell === //

/// An [RwLock](std::sync::RwLock) without blocking capabilities. Users are much better off using a
/// dedicated scheduler to handle resource access synchronization because blocking [RwLock]s do not
/// provide the semantics required to implement true resource lock scheduling (system dependencies are
/// expressed as unordered sets; `Mutexes` and `RwLocks` must have a well defined lock order to avoid
/// dead-locks).
pub struct ARefCell<T: ?Sized> {
	counter: LockCounter,
	value: UnsafeCell<T>,
}

impl<T: Debug> Debug for ARefCell<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut rw_guard: Option<ARef<T>> = None;

		let value_debug: &dyn Debug = self
			.try_borrow()
			.map_or(&"cannot inspect without blocking", |guard| {
				rw_guard.insert(guard)
			});

		f.debug_struct("ARefCell")
			.field("counter", &self.counter)
			.field("value", value_debug)
			.finish()
	}
}

impl<T: Clone> Clone for ARefCell<T> {
	fn clone(&self) -> Self {
		Self::new(self.borrow().clone())
	}
}

impl<T: Default> Default for ARefCell<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T, U> CoerceUnsized<ARefCell<U>> for ARefCell<T> where T: CoerceUnsized<U> {}

unsafe impl<T: ?Sized + Send> Send for ARefCell<T> {}
unsafe impl<T: ?Sized + Sync> Sync for ARefCell<T> {}

impl<T> ARefCell<T> {
	pub fn new(value: T) -> Self {
		Self {
			counter: LockCounter::default(),
			value: UnsafeCell::new(value),
		}
	}

	pub fn into_inner(self) -> T {
		self.value.into_inner()
	}
}

impl<T: ?Sized> ARefCell<T> {
	pub fn as_ptr(&self) -> *mut T {
		self.value.get()
	}

	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}

	pub fn undo_leak(&mut self) {
		self.counter.undo_leak();
	}

	pub fn lock_state_snapshot(&self) -> LockState {
		self.counter.snapshot()
	}

	// === Immutable Borrow === //

	pub unsafe fn try_borrow_unsynchronized(&self) -> Result<ARef<T>, LockError> {
		let guard = ReadGuard::try_lock_unsynchronized(&self.counter)?;
		Ok(ARef {
			guard,
			value: &*self.value.get(),
		})
	}

	pub unsafe fn borrow_unsynchronized(&self) -> ARef<T> {
		self.try_borrow_unsynchronized().unwrap_pretty()
	}

	pub fn try_borrow(&self) -> Result<ARef<T>, LockError> {
		let guard = ReadGuard::try_lock(&self.counter)?;
		Ok(ARef {
			guard,
			value: unsafe { &*self.value.get() },
		})
	}

	pub fn borrow(&self) -> ARef<T> {
		self.try_borrow().unwrap_pretty()
	}

	// === Mutable Borrow === //

	pub unsafe fn try_borrow_unsynchronized_mut(&self) -> Result<AMut<T>, LockError> {
		let guard = WriteGuard::try_lock_unsynchronized(&self.counter)?;
		Ok(AMut {
			guard,
			value: &mut *self.value.get(),
		})
	}

	pub unsafe fn borrow_unsynchronized_mut(&self) -> AMut<T> {
		self.try_borrow_unsynchronized_mut().unwrap_pretty()
	}

	pub fn try_borrow_mut(&self) -> Result<AMut<T>, LockError> {
		let guard = WriteGuard::try_lock(&self.counter)?;
		Ok(AMut {
			guard,
			value: unsafe { &mut *self.value.get() },
		})
	}

	pub fn borrow_mut(&self) -> AMut<T> {
		self.try_borrow_mut().unwrap_pretty()
	}
}

pub struct ARef<'a, T: ?Sized> {
	guard: ReadGuard<'a>,
	value: &'a T,
}

impl<'a, T: ?Sized> ARef<'a, T> {
	pub fn clone_ref(target: &Self) -> Self {
		Self {
			guard: target.guard.clone(),
			value: target.value,
		}
	}

	pub fn map<U, F>(target: Self, f: F) -> ARef<'a, U>
	where
		F: FnOnce(&T) -> &U,
		U: ?Sized,
	{
		let Self { guard, value } = target;
		ARef {
			guard,
			value: f(value),
		}
	}

	pub fn filter_map<U, F>(target: Self, f: F) -> Result<ARef<'a, U>, ARef<'a, T>>
	where
		F: FnOnce(&T) -> Option<&U>,
		U: ?Sized,
	{
		let value = match f(target.value) {
			Some(value) => value,
			None => return Err(target),
		};
		let borrow = target.guard;

		Ok(ARef {
			guard: borrow,
			value,
		})
	}

	pub fn map_slit<U, V, F>(target: Self, f: F) -> (ARef<'a, U>, ARef<'a, V>)
	where
		F: FnOnce(&T) -> (&U, &V),
		U: ?Sized,
		V: ?Sized,
	{
		let Self { guard, value } = target;
		let (left, right) = f(value);

		let left = ARef {
			guard: guard.clone(),
			value: left,
		};
		let right = ARef {
			guard,
			value: right,
		};

		(left, right)
	}

	pub fn leak(target: Self) -> &'a T {
		let Self { guard, value } = target;
		std::mem::forget(guard);
		value
	}
}

impl<'a, T, U> CoerceUnsized<ARef<'a, U>> for ARef<'a, T>
where
	T: Unsize<U> + ?Sized,
	U: ?Sized,
{
}

impl<T: ?Sized> Deref for ARef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.value
	}
}

impl<T: ?Sized + Debug> Debug for ARef<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: ?Sized + Display> Display for ARef<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

pub struct AMut<'a, T: ?Sized> {
	guard: WriteGuard<'a>,
	value: &'a mut T,
}

impl<'a, T: ?Sized> AMut<'a, T> {
	pub fn map<U, F>(target: Self, f: F) -> AMut<'a, U>
	where
		F: FnOnce(&mut T) -> &mut U,
		U: ?Sized,
	{
		let Self { guard, value } = target;
		AMut {
			guard,
			value: f(value),
		}
	}

	pub fn filter_map<U, F>(target: Self, f: F) -> Result<AMut<'a, U>, AMut<'a, T>>
	where
		F: FnOnce(&mut T) -> Option<&mut U>,
		U: ?Sized,
	{
		let value = {
			// We need to make the value reference unbounded because the borrow checker assumes that
			// the reborrow passed to `f` will have to live for `'a'`, even if we end up taking the
			// early return.
			let value_ref_unbounded = unsafe {
				let ptr = target.value as *mut _;
				&mut *ptr
			};

			match f(value_ref_unbounded) {
				Some(value) => value,
				None => return Err(target),
			}
		};
		let guard = target.guard;

		Ok(AMut { guard, value })
	}

	pub fn map_slit<U, V, F>(target: Self, f: F) -> (AMut<'a, U>, AMut<'a, V>)
	where
		F: FnOnce(&mut T) -> (&mut U, &mut V),
		U: ?Sized,
		V: ?Sized,
	{
		let Self { guard, value } = target;
		let (left, right) = f(value);

		let left = AMut {
			guard: guard.clone(),
			value: left,
		};
		let right = AMut {
			guard,
			value: right,
		};

		(left, right)
	}

	pub fn leak(target: Self) -> &'a mut T {
		let Self { guard, value } = target;
		std::mem::forget(guard);
		value
	}
}

impl<'a, T, U> CoerceUnsized<AMut<'a, U>> for AMut<'a, T>
where
	T: Unsize<U> + ?Sized,
	U: ?Sized,
{
}

impl<T: ?Sized> Deref for AMut<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.value
	}
}

impl<T: ?Sized> DerefMut for AMut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.value
	}
}

impl<T: ?Sized + Debug> Debug for AMut<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: ?Sized + Display> Display for AMut<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}
