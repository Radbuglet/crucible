use crate::util::error::ResultExt;
use crate::util::usually::UsuallySafeCell;
use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::Unsize;
use std::ops::{CoerceUnsized, Deref, DerefMut};
use std::sync::atomic::{AtomicIsize, Ordering as AtomicOrdering};

const TOO_MANY_REFS_ERROR: &str =
	"Cannot create more than isize::MAX concurrent references to an `ARefCell`.";

/// An [RwLock](std::sync::RwLock) without blocking capabilities. Users are much better off using a
/// dedicated scheduler to handle resource access synchronization as blocking [RwLock]s do not provide
/// the semantics required to implement true resource lock scheduling (system dependencies are expressed
/// as unordered sets; `Mutexes` and `RwLocks` must have a well defined lock order to avoid dead-locks).
pub struct ARefCell<T: ?Sized> {
	rc: UsuallySafeCell<AtomicIsize>,
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
			.field("rc", &self.lock_state_snapshot())
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
			rc: UsuallySafeCell::new(AtomicIsize::new(0)),
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
		*self.rc.get_mut() = 0;
	}

	pub fn lock_state_snapshot(&self) -> LockState {
		LockState::from_state(self.rc.load(AtomicOrdering::Relaxed))
	}

	// === Immutable Borrow === //

	pub unsafe fn try_borrow_unsynchronized(&self) -> Result<ARef<T>, LockError> {
		let rc = self.rc.unchecked_get_mut().get_mut();

		if *rc <= 0 {
			*rc = rc.checked_sub(1).expect(TOO_MANY_REFS_ERROR);
			Ok(ARef {
				borrow: ABorrow(&self.rc),
				value: &*self.value.get(),
			})
		} else {
			Err(LockError {
				state: LockState::from_state(*rc),
			})
		}
	}

	pub unsafe fn borrow_unsynchronized(&self) -> ARef<T> {
		self.try_borrow_unsynchronized().unwrap_pretty()
	}

	pub fn try_borrow(&self) -> Result<ARef<T>, LockError> {
		let result = self
			.rc
			.fetch_update(AtomicOrdering::Acquire, AtomicOrdering::Relaxed, |rc| {
				if rc <= 0 {
					Some(rc.checked_sub(1).expect(TOO_MANY_REFS_ERROR))
				} else {
					None
				}
			});

		match result {
			Ok(_) => Ok(ARef {
				borrow: ABorrow(&self.rc),
				value: unsafe { &*self.value.get() },
			}),
			Err(rc) => Err(LockError {
				state: LockState::from_state(rc),
			}),
		}
	}

	pub fn borrow(&self) -> ARef<T> {
		self.try_borrow().unwrap_pretty()
	}

	// === Mutable Borrow === //

	pub unsafe fn try_borrow_unsynchronized_mut(&self) -> Result<AMut<T>, LockError> {
		let rc = self.rc.unchecked_get_mut().get_mut();

		if *rc == 0 {
			*rc = 1;
			Ok(AMut {
				borrow: ABorrow(&self.rc),
				value: &mut *self.value.get(),
			})
		} else {
			Err(LockError {
				state: LockState::from_state(*rc),
			})
		}
	}

	pub unsafe fn borrow_unsynchronized_mut(&self) -> AMut<T> {
		self.try_borrow_unsynchronized_mut().unwrap_pretty()
	}

	pub fn try_borrow_mut(&self) -> Result<AMut<T>, LockError> {
		let result = self
			.rc
			.fetch_update(AtomicOrdering::Acquire, AtomicOrdering::Relaxed, |rc| {
				if rc == 0 {
					Some(1)
				} else {
					None
				}
			});

		match result {
			Ok(_) => Ok(AMut {
				borrow: ABorrow(&self.rc),
				value: unsafe { &mut *self.value.get() },
			}),
			Err(rc) => Err(LockError {
				state: LockState::from_state(rc),
			}),
		}
	}

	pub fn borrow_mut(&self) -> AMut<T> {
		self.try_borrow_mut().unwrap_pretty()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum LockState {
	Mutably(usize),
	Immutably(usize),
	Unborrowed,
}

impl LockState {
	fn from_state(value: isize) -> Self {
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

type ABorrowRef<'a> = ABorrow<'a, { -1 }>;
type ABorrowMut<'a> = ABorrow<'a, 1>;

struct ABorrow<'a, const DELTA: isize>(&'a AtomicIsize);

impl<const DELTA: isize> Clone for ABorrow<'_, DELTA> {
	fn clone(&self) -> Self {
		let success = self
			.0
			.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |val| {
				val.checked_add(DELTA)
			})
			.is_ok();

		assert!(success, "{}", TOO_MANY_REFS_ERROR);

		Self(self.0)
	}
}

impl<const DELTA: isize> Drop for ABorrow<'_, DELTA> {
	fn drop(&mut self) {
		self.0.fetch_sub(DELTA, AtomicOrdering::Release);
	}
}

pub struct ARef<'a, T: ?Sized> {
	borrow: ABorrowRef<'a>,
	value: &'a T,
}

impl<'a, T: ?Sized> ARef<'a, T> {
	pub fn clone_ref(target: &Self) -> Self {
		Self {
			borrow: target.borrow.clone(),
			value: target.value,
		}
	}

	pub fn map<U, F>(target: Self, f: F) -> ARef<'a, U>
	where
		F: FnOnce(&T) -> &U,
		U: ?Sized,
	{
		let Self { borrow, value } = target;
		ARef {
			borrow,
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
		let borrow = target.borrow;

		Ok(ARef { borrow, value })
	}

	pub fn map_slit<U, V, F>(target: Self, f: F) -> (ARef<'a, U>, ARef<'a, V>)
	where
		F: FnOnce(&T) -> (&U, &V),
		U: ?Sized,
		V: ?Sized,
	{
		let Self { borrow, value } = target;
		let (left, right) = f(value);

		let left = ARef {
			borrow: borrow.clone(),
			value: left,
		};
		let right = ARef {
			borrow,
			value: right,
		};

		(left, right)
	}

	pub fn leak(target: Self) -> &'a T {
		let Self { borrow, value } = target;
		std::mem::forget(borrow);
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
	borrow: ABorrowMut<'a>,
	value: &'a mut T,
}

impl<'a, T: ?Sized> AMut<'a, T> {
	pub fn map<U, F>(target: Self, f: F) -> AMut<'a, U>
	where
		F: FnOnce(&mut T) -> &mut U,
		U: ?Sized,
	{
		let Self { borrow, value } = target;
		AMut {
			borrow,
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
		let borrow = target.borrow;

		Ok(AMut { borrow, value })
	}

	pub fn map_slit<U, V, F>(target: Self, f: F) -> (AMut<'a, U>, AMut<'a, V>)
	where
		F: FnOnce(&mut T) -> (&mut U, &mut V),
		U: ?Sized,
		V: ?Sized,
	{
		let Self { borrow, value } = target;
		let (left, right) = f(value);

		let left = AMut {
			borrow: borrow.clone(),
			value: left,
		};
		let right = AMut {
			borrow,
			value: right,
		};

		(left, right)
	}

	pub fn leak(target: Self) -> &'a mut T {
		let Self { borrow, value } = target;
		std::mem::forget(borrow);
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
