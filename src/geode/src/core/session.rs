use std::{cell::Cell, hint::unreachable_unchecked, marker::PhantomData};

use arr_macro::arr;
use parking_lot::Mutex;

use crate::util::{
	cell::MutexedUnsafeCell,
	marker::{PhantomNoSendOrSync, PhantomNoSync},
	number::U8BitSet,
	ptr::dangerous_transmute,
	threading::new_lot_mutex,
};

// === Global State === //

/// Session ID allocator.
static ID_ALLOC: Mutex<U8BitSet> = new_lot_mutex(U8BitSet::new());

struct UnregisterGuard(u8);

impl Drop for UnregisterGuard {
	fn drop(&mut self) {
		ID_ALLOC.lock().unset(self.0);
	}
}

/// Allocates a new session.
///
/// ## Panics
///
/// Panics if a session initializer panics or too many sessions have been created. If it panics
/// during initialization, no IDs will be leaked.
///
fn allocate_session() -> u8 {
	// Allocate ID
	let id = ID_ALLOC.lock().reserve_zero_bit().unwrap_or(0xFF);
	assert_ne!(id, 0xFF, "Cannot create more than 255 sessions!");

	// Setup guard to unregister ID if something goes poorly.
	// We complete the transaction before knowing whether it is valid because `init_session` can call
	// to `allocate_session`.
	let unregister_guard = UnregisterGuard(id);

	// Initialize all critical session info instances.
	unsafe {
		// Safety: this is a new session to which no one else has access.
		<() as StaticStorageHygieneBreak>::init_session(id);
	}

	// Defuse the `unregister_guard`—we don't need it anymore.
	std::mem::forget(unregister_guard);

	id
}

/// Deallocates an existing session.
fn dealloc_session(id: u8) {
	// We set up a guard to unregister the session `id` once `deinit_session` finishes or panics.
	// We cannot unregister the session until `deinit_session` has stopped because the handler might
	// then allocate a session with that free ID that is simultaneously being initialized and
	// deinitialized, which would cause the *big bad*.
	let unregister_guard = UnregisterGuard(id);

	unsafe {
		// Safety: this is an old session to which everyone has given up access.
		<() as StaticStorageHygieneBreak>::deinit_session(id);
	}

	drop(unregister_guard);
}

// === Sessions === //

#[thread_local]
static LOCAL_SESSION: Cell<LocalSessionInfo> = Cell::new(LocalSessionInfo { id: 0xFF, rc: 0 });

#[derive(Copy, Clone)]
struct LocalSessionInfo {
	id: u8,
	rc: u64,
}

// Movable
#[derive(Debug)]
pub struct MovableSessionGuard {
	_no_sync: PhantomNoSync,
	id: u8,
}

impl MovableSessionGuard {
	pub fn new() -> Self {
		Self {
			_no_sync: PhantomData,
			id: allocate_session(),
		}
	}

	pub fn handle(&self) -> Session<'_> {
		Session::new_internal(self.id)
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
		// session instance into a `LocalSessionGuard`.
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
		Session::new_internal(self.id)
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
	fn new_internal(id: u8) -> Self {
		Self {
			_lifetime: PhantomData,
			_no_threading: PhantomData,
			id,
		}
	}

	pub fn slot(self) -> u8 {
		debug_assert_ne!(self.id, 0xFF);
		self.id
	}
}

// === Session Storage === //

pub type OptionSessionStorage<T> = SessionStorage<Option<T>>;

pub struct SessionStorage<T> {
	slots: [MutexedUnsafeCell<T>; 256],
}

impl<T> OptionSessionStorage<T> {
	pub const fn new() -> Self {
		Self::new_with(arr![None; 256])
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
	raw: OptionSessionStorage<T>,
}

impl<T> LazySessionStorage<T> {
	pub const fn new() -> Self {
		Self {
			raw: OptionSessionStorage::new(),
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

// === Session Init Registry === //

/// An internal macro hygiene-break trait that [register_static_storages] implements on `()`. Doing
/// things this way also ensures that we can only run the macro once.
///
/// ## Safety
///
/// Only [register_static_storages] can safely implement this trait.
///
unsafe trait StaticStorageHygieneBreak {
	/// Runs all session initializers for a session with the specified `id`.
	///
	/// ## Panics
	///
	/// Panics if one of the user-supplied session initializers panics.
	///
	/// ## Safety
	///
	/// Only supply a session for which no [Session] handles are available to untrusted code.
	///
	unsafe fn init_session(id: u8);

	/// Runs all session initializers for a session with the specified `id`.
	///
	/// ## Panics
	///
	/// Panics if one of the user-supplied session initializers panics.
	///
	/// ## Safety
	///
	/// Only supply a session for which no [Session] handles are available to untrusted code and
	/// which has been fully initialized by [init_session](StaticStorageHygieneBreak::init_session)
	/// without panicking.
	///
	unsafe fn deinit_session(id: u8);
}

/// A project-internal trait specifying that a given struct should be turned into a greedily
/// initialized [SessionStorage]. Access to the underlying storage is provided by the unsafe
/// [StaticStorage] trait, which can only be implemented by registering the target struct using the
/// [register_static_storages] macro.
///
/// ## Reentrancy and Panics
///
/// [init_comp] and [deinit_comp] run on the thread that created the session and are free to perform
/// session management of their own. If either of them panic, the panic will be forwarded to the user.
///
/// [init_comp]: StaticStorageHandler::init_comp
/// [deinit_comp]: StaticStorageHandler::deinit_comp
pub(crate) trait StaticStorageHandler {
	/// The type of the component stored in the storage.
	type Comp: Sized + 'static;

	/// Initializes the provided component slot. `target` must be set to `Some(_)` by the time the
	/// function returns. `target` will start out as `None` when the program starts up but may be
	/// `Some(_)` if [deinit_comp](StaticStorageHandler::deinit_comp) failed to set the target to
	/// `None` (either because it chose to do so or because a prior [StaticStorageHandler] panicked
	/// while deinitializing its own component slot).
	fn init_comp(target: &mut Option<Self::Comp>);

	/// Deinitializes the provided component slot. `target` does *not* have to be set to `None` by
	/// the time the function returns, and doing so may be helpful when implementing session reuse.
	/// The deinitializer may not run if a prior deinitializer panics.
	///
	/// TODO: We might need stronger semantics here (e.g. a try-catch-finally system) to ensure that
	/// leaks in other systems don't cause leaks in ours.
	fn deinit_comp(_target: &mut Option<Self::Comp>) {
		// (no op)
	}
}

/// A trait providing static methods to access the statically-initialized [SessionStorage] attached
/// to the item on which the trait is implemented.
///
/// **This trait is not to be implemented manually.** Rather, users should implement [StaticStorageHandler]
/// to define the actual storage's properties and then derive this trait by registering the struct in
/// the singleton call to [register_static_storages].
pub(crate) unsafe trait StaticStorage: StaticStorageHandler {
	unsafe fn backing_storage() -> &'static SessionStorage<Option<Self::Comp>>;

	#[inline(always)]
	fn get<'a>(session: Session<'a>) -> &'a Self::Comp {
		unsafe {
			match Self::backing_storage().get(session) {
				Some(comp) => comp,
				None => unreachable_unchecked(),
			}
		}
	}
}

macro register_static_storages($($target:path),*$(,)?) {
	unsafe impl StaticStorageHygieneBreak for () {
		unsafe fn init_session(id: u8) {
			$({
				// Safety: trust us, we're professionals. (this method is unsafe just to make sure
				// that external users keep away from our stuff)
				let storage = <$target as StaticStorage>::backing_storage();

				// Safety: We're accessing this state before anyone else even has access to this
				// session and we release the reference before anyone else gets to read it.
				let state = storage.get_mut_unchecked(Session::new_internal(id));

				// Initialize the state and ensure that the user hasn't messed anything up.
				<$target as StaticStorageHandler>::init_comp(state);
				assert!(state.is_some(), "`{}::init_comp` failed to initialize component.", stringify!($target));
			};)*
		}

		unsafe fn deinit_session(id: u8) {
			$({
				// Safety: trust us, we're professionals. (this method is unsafe just to make sure
				// that external users keep away from our stuff)
				let storage = <$target as StaticStorage>::backing_storage();

				// Safety: We're accessing this state after everyone else has given up access to it.
				let state = storage.get_mut_unchecked(Session::new_internal(id));

				// As a gesture of kindness, we tell the compiler that the state is not `None` at this
				// point so the user can unwrap it for free.
				match state {
					Some(_) => {}
					// Safety: if this invariant didn't hold up, we'd be dead long ago.
					None => unreachable_unchecked(),
				}

				// Users can do whatever they want here.
				<$target as StaticStorageHandler>::deinit_comp(state);
			};)*
		}
	}

	$(
		unsafe impl StaticStorage for $target {
			#[inline(always)]
			unsafe fn backing_storage() -> &'static SessionStorage<Option<Self::Comp>> {
				static STORAGE: OptionSessionStorage<<$target as StaticStorageHandler>::Comp> = OptionSessionStorage::new();
				&STORAGE
			}
		}
	)*
}

register_static_storages!(super::obj::LocalSessData);
