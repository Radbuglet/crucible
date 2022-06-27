use std::{cell::Cell, marker::PhantomData};

use arr_macro::arr;
use parking_lot::Mutex;

use crate::util::{
	cell::{AssumeInit, MutexedUnsafeCell},
	marker::{PhantomNoSendOrSync, PhantomNoSync},
	number::U8BitSet,
	ptr::dangerous_transmute,
	threading::new_lot_mutex,
};

// === Session state (de)initialization routines === //

// TODO: We might even want to write a macro to enforce (de)initialization in a safe manner.

/// Initializes all [InitSessionStorage] containers contained within Geode.
///
/// ## Safety
///
/// Don't be stupid.
///
fn init_session_states(session: Session<'_>) {
	crate::core::obj::init_session_states(session);
}

fn deinit_session_states(session: Session<'_>) {
	crate::core::obj::deinit_session_states(session);
}

// === Global State === //

/// Session ID allocator.
static ID_ALLOC: Mutex<U8BitSet> = new_lot_mutex(U8BitSet::new());

/// Deallocates an existing session.
fn dealloc_session(id: u8) {
	ID_ALLOC.lock().unset(id);
	deinit_session_states(Session {
		_lifetime: PhantomData,
		_no_threading: PhantomData,
		id,
	});
}

// === Sessions === //

// Movable
#[derive(Debug)]
pub struct MovableSessionGuard {
	_no_sync: PhantomNoSync,
	id: u8,
}

impl MovableSessionGuard {
	pub fn new() -> Self {
		// Allocate ID
		let id = ID_ALLOC.lock().reserve_zero_bit().unwrap_or(0xFF);
		assert_ne!(id, 0xFF, "Cannot create more than 255 sessions!");

		// Initialize all critical session info instances.
		init_session_states(Session {
			_lifetime: PhantomData,
			_no_threading: PhantomData,
			id,
		});

		// Construct guard
		Self {
			_no_sync: PhantomData,
			id,
		}
	}

	pub fn handle(&self) -> Session<'_> {
		Session {
			_lifetime: PhantomData,
			_no_threading: PhantomData,
			id: self.id,
		}
	}

	pub fn make_local(self) -> LocalSessionGuard {
		// Ensure that there isn't already a session on this thread.
		assert_eq!(
			LOCAL_SESSION.get().rc,
			0,
			"Cannot call `make_local` if the current thread already has a local thread."
		);

		// Update the local session
		let id = self.id;
		LOCAL_SESSION.set(LocalSessionInfo { id, rc: 1 });

		// Ensure that we don't run our destructor since we're effectively transforming this
		// session's type.
		std::mem::forget(self);

		// Construct handle
		LocalSessionGuard {
			_no_threading: PhantomData,
			id,
		}
	}
}

impl Drop for MovableSessionGuard {
	fn drop(&mut self) {
		dealloc_session(self.id);
	}
}

// Local
#[thread_local]
static LOCAL_SESSION: Cell<LocalSessionInfo> = Cell::new(LocalSessionInfo { id: 0xFF, rc: 0 });

#[derive(Copy, Clone)]
struct LocalSessionInfo {
	id: u8,
	rc: u64,
}

#[derive(Debug)]
pub struct LocalSessionGuard {
	_no_threading: PhantomNoSendOrSync,
	id: u8,
}

impl LocalSessionGuard {
	#[inline(always)]
	pub fn new() -> Self {
		Self::with_new(|session| session)
	}

	#[inline(always)]
	pub fn with_new<F, R>(mut f: F) -> R
	where
		F: FnMut(Self) -> R,
	{
		if let Some(reused) = Self::try_reuse() {
			f(reused)
		} else {
			Self::with_new_cold(f)
		}
	}

	#[cold]
	#[inline(never)]
	fn with_new_cold<F, R>(mut f: F) -> R
	where
		F: FnMut(Self) -> R,
	{
		let session = MovableSessionGuard::new().make_local();
		f(session)
	}

	#[inline(always)]
	pub fn try_reuse() -> Option<Self> {
		let mut copy = LOCAL_SESSION.get();

		if copy.rc > 0 {
			copy.rc += 1;
			LOCAL_SESSION.set(copy);

			Some(LocalSessionGuard {
				_no_threading: PhantomData,
				id: copy.id,
			})
		} else {
			None
		}
	}

	#[inline(always)]
	pub fn handle(&self) -> Session<'_> {
		Session {
			_lifetime: PhantomData,
			_no_threading: PhantomData,
			id: self.id,
		}
	}
}

impl Drop for LocalSessionGuard {
	fn drop(&mut self) {
		#[cold]
		#[inline(never)]
		fn drop_cold(id: u8) {
			dealloc_session(id)
		}

		let mut copy = LOCAL_SESSION.get();

		copy.rc -= 1;
		LOCAL_SESSION.set(copy);

		if copy.rc == 0 {
			drop_cold(copy.id);
		}
	}
}

// Session handle
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Session<'a> {
	_lifetime: PhantomData<&'a MovableSessionGuard>,
	_no_threading: PhantomNoSendOrSync,
	id: u8,
}

impl Session<'_> {
	pub fn slot(self) -> u8 {
		debug_assert_ne!(self.id, 0xFF);
		self.id
	}
}

// === Session Storage === //

pub(crate) type InitSessionStorage<T> = SessionStorage<AssumeInit<T>>;

pub struct SessionStorage<T> {
	slots: [MutexedUnsafeCell<T>; 256],
}

impl<T> InitSessionStorage<T> {
	pub(crate) const unsafe fn new() -> Self {
		// Safety: the users promise to initialize these before we return references to them.
		Self::new_with(arr![AssumeInit::uninit(); 256])
	}
}

impl<T> SessionStorage<T> {
	pub const fn new_with(arr: [T; 256]) -> Self {
		Self {
			slots: unsafe {
				// Safety: `MutexedUnsafeCell` is `repr(transparent)` so the two types have the same
				// layout. These two types are not wrapped in anything so we're not susceptible to
				// e.g. the super dangerous `&T` to `&UnsafeCell<T>` cast.
				dangerous_transmute::<[T; 256], [MutexedUnsafeCell<T>; 256]>(arr)
			},
		}
	}

	#[inline(always)]
	pub fn get<'a>(&'a self, session: Session<'a>) -> &'a T {
		unsafe {
			// Safety: we know, by the fact that `session` cannot be shared between threads, that
			// we are on the only thread with access to this value.
			self.slots[session.slot() as usize].get_ref_unchecked()
		}
	}

	#[inline(always)]
	pub unsafe fn get_mut_unchecked<'a>(&'a self, session: Session<'a>) -> &'a mut T {
		// Safety: a combination of the validity of `.get` with an additional user-supplied
		// guarantee that no one else on the same thread is borrowing the value simultaneously.
		self.slots[session.slot() as usize].get_mut_unchecked()
	}
}

pub struct LazySessionStorage<T> {
	raw: SessionStorage<Option<T>>,
}

impl<T> LazySessionStorage<T> {
	pub const fn new() -> Self {
		Self {
			raw: SessionStorage::new_with(arr![None; 256]),
		}
	}

	#[inline(always)]
	pub fn get<'a>(&'a self, session: Session<'a>) -> Option<&'a T> {
		self.raw.get(session).as_ref()
	}

	#[inline(always)]
	pub fn get_or_init_using<'a, F>(&'a self, session: Session<'a>, initializer: F) -> &'a T
	where
		F: FnMut() -> T,
	{
		// Try to acquire via existing reference
		if let Some(data) = self.get(session) {
			data
		} else {
			self.init_cold(session, initializer)
		}
	}

	#[cold]
	#[inline(never)]
	fn init_cold<'a, F>(&'a self, session: Session<'a>, mut initializer: F) -> &'a T
	where
		F: FnMut() -> T,
	{
		// Run our initializer
		let value = initializer();

		// Ensure that our initializer has not already initialized the value.
		assert!(
			self.get(session).is_none(),
			"`initializer` cannot call `get_or_init` on its own storage."
		);

		// Initialize and return
		unsafe {
			// Safety: we know that no references to the `Option` because it is still `None` and
			// we only return references to the inner value of the `Option` if it is not `None`.
			// Because we hava session for this slot, we can assume that our thread has exclusive
			// access to this slot.
			// Thus, this is safe.
			let slot = self.raw.get_mut_unchecked(session);

			// This cannot run a destructor to observe the mutable borrow because we already checked
			// that it was none.
			*slot = Some(value);

			// Safety: we just need to make sure to return an immutable reference now.
			slot.as_ref().unwrap()
		}
	}
}

impl<T: Default> LazySessionStorage<T> {
	#[inline(always)]
	pub fn get_or_init<'a>(&'a self, session: Session<'a>) -> &'a T {
		self.get_or_init_using(session, Default::default)
	}
}
