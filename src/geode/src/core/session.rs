use std::{
	cell::{Cell, UnsafeCell},
	fmt, hash,
	hint::unreachable_unchecked,
	marker::PhantomData,
};

use crucible_core::{
	array::{arr, arr_indexed},
	cell::{MutexedUnsafeCell, UnsafeCellExt},
	marker::{PhantomNoSendOrSync, PhantomNoSync},
	sync::AssertSync,
	transmute::sizealign_checked_transmute,
};
use parking_lot::Mutex;

use crate::util::{number::U8BitSet, threading::new_lot_mutex};

// === Global State === //

/// Session ID allocator.
static ID_ALLOC: Mutex<U8BitSet> = new_lot_mutex(U8BitSet::new());

/// A guard that unregisters a session with the provided ID on `Drop`.
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
/// ## Safety
///
/// `PrimaryStorageEntry` truly only lives until the session is deallocated.
///
fn allocate_session() -> &'static PrimaryStorageEntry {
	// Allocate ID and set up an unregistry guard to trigger on panic. We mark the session ID as
	// registered here and employ a guard because `init_session` can call `allocate_session` (making
	// this function reentrant) and we need to prevent that semi-initialized ID from being reused.
	let id = ID_ALLOC.lock().reserve_zero_bit().unwrap_or(0xFF);
	assert_ne!(id, 0xFF, "Cannot create more than 254 sessions!"); // TODO: Do we need this cap?

	let unregister_guard = UnregisterGuard(id);

	// Initialize all critical session info instances.
	let primary_ptr = unsafe {
		// Safety: this is a new session to which no one else has access.
		<() as StaticStorageHygieneBreak>::init_session(id)
	};

	// Defuse the `unregister_guard`. A user-facing guard designed to call `dealloc_session` will be
	// created in its place.
	std::mem::forget(unregister_guard);

	primary_ptr
}

/// Deallocates an existing session.
fn dealloc_session(id: u8) {
	// We set up a guard to unregister the session `id` once `deinit_session` finishes or panics.
	// We cannot unregister the session until `deinit_session` has finished because the handler might
	// then allocate a session with that free ID that is simultaneously being initialized and
	// deinitialized, which could cause the *big bad*.
	let unregister_guard = UnregisterGuard(id);

	unsafe {
		// Safety: this is an old session to which everyone has given up access.
		<() as StaticStorageHygieneBreak>::deinit_session(id);
	}

	drop(unregister_guard);
}

// === Sessions === //

#[derive(Copy, Clone)]
struct LocalSessionInfo {
	ptr: Option<&'static PrimaryStorageEntry>,
	rc: u64,
}

#[thread_local]
static LOCAL_SESSION: Cell<LocalSessionInfo> = Cell::new(LocalSessionInfo { ptr: None, rc: 0 });

// Movable
pub struct MovableSessionGuard {
	// `ptr` is actually `Sync` for convenience sake. See `PrimaryStorageEntryOf`'s item comment for
	// details.
	_no_sync: PhantomNoSync,
	ptr: &'static PrimaryStorageEntry,
}

impl fmt::Debug for MovableSessionGuard {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("MovableSessionGuard")
			.field("slot", &self.handle().slot())
			.finish()
	}
}

impl Default for MovableSessionGuard {
	fn default() -> Self {
		Self::new()
	}
}

impl MovableSessionGuard {
	pub fn new() -> Self {
		Self {
			_no_sync: PhantomData,
			ptr: allocate_session(),
		}
	}

	/// Acquires a bunch of sessions atomically as specified by a filter. The filter is provided
	/// a [u8] indicating the ID of the session slot and a [bool] indicating whether it is free.
	/// If the `free` boolean flag is set, the filter can acquire the session by returning `Ok(true)`.
	/// Returning `Ok(false)` will leave the session slot unaffected. The filter may abort the
	/// operation at any time by returning an [Err], which will be forwarded on to the caller.
	/// Panicking is also valid.
	///
	/// ## Locking
	///
	/// To make this method atomic, the method acquires a global mutex for the duration it runs the
	/// acquire filter. The returned iterator does not hold this mutex. Because of this, the provided
	/// filter closure must not call other methods that also acquire this mutex (i.e. all session
	/// constructors and destructors) or block on the completion of methods also acquiring this
	/// session slot.
	///
	/// Filtering sessions by providing a proper filtering closure to this method, and filtering
	/// sessions by acquiring all of them and then discarding [MovableSessionGuard] instances that
	/// don't fit the filter, are seemingly equivalent but subtly different. Because the iterator
	/// returned by `acquire_many` does not hold onto the global session DB mutex, other users may be
	/// able to slot in their call to [MovableSessionGuard::new] while the user is still filtering
	/// their sessions. If the original closure is too aggressive in its reservation, it may exhaust
	/// the strict `255` session limit, causing this interwoven constructor call—that could be
	/// otherwise valid given a different ordering—to spuriously fail.
	///
	pub fn acquire_many<F, E>(mut filter: F) -> Result<SessionManyAllocIter, E>
	where
		F: FnMut(u8, bool) -> Result<bool, E>,
	{
		let mut ids = ID_ALLOC.lock();
		let mut requested = U8BitSet::new();

		// Filter IDs
		for id in 0..255 {
			let available = ids.contains(id);
			if filter(id, available)? {
				assert!(
					available,
					"Attempted to acquire unavailable session with ID ({id})"
				);

				requested.set(id);
			}
		}

		// Apply requested mask
		ids.bitwise_or(&requested);

		// Create the iterator to produce the acquired session IDs.
		Ok(SessionManyAllocIter {
			remaining: requested,
		})
	}

	pub fn handle(&self) -> Session<'_> {
		Session { ptr: self.ptr }
	}

	pub fn make_local(self) -> LocalSessionGuard {
		// Ensure that there isn't already a session on this thread.
		assert_eq!(
			LOCAL_SESSION.get().rc,
			0,
			"Cannot call `make_local` if the current thread already has a local thread."
		);
		debug_assert!(LOCAL_SESSION.get().ptr.is_none());

		// Update the local session
		let ptr = self.ptr;
		LOCAL_SESSION.set(LocalSessionInfo {
			ptr: Some(ptr),
			rc: 1,
		});

		// Ensure that we don't run our destructor since we're effectively transforming this
		// session instance into a `LocalSessionGuard`.
		std::mem::forget(self);

		// Construct handle
		LocalSessionGuard {
			_no_threading: PhantomData,
			ptr,
		}
	}
}

impl Drop for MovableSessionGuard {
	fn drop(&mut self) {
		dealloc_session(self.handle().slot());
	}
}

#[derive(Debug)]
pub struct SessionManyAllocIter {
	remaining: U8BitSet,
}

impl Iterator for SessionManyAllocIter {
	type Item = MovableSessionGuard;

	fn next(&mut self) -> Option<Self::Item> {
		let id = self.remaining.reserve_set_bit()?;

		let ptr = unsafe {
			// Safety: this is a new session to which no one else has access.
			<() as StaticStorageHygieneBreak>::init_session(id)
		};

		Some(MovableSessionGuard {
			_no_sync: PhantomData,
			ptr,
		})
	}
}

// Local
pub struct LocalSessionGuard {
	_no_threading: PhantomNoSendOrSync,
	ptr: &'static PrimaryStorageEntry,
}

impl fmt::Debug for LocalSessionGuard {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LocalSessionGuard")
			.field("slot", &self.handle().slot())
			.finish()
	}
}

impl Default for LocalSessionGuard {
	fn default() -> Self {
		Self::new()
	}
}

impl LocalSessionGuard {
	#[inline(always)]
	pub fn new() -> Self {
		Self::with_new(|session| session)
	}

	#[inline(always)]
	pub fn with_new<F, R>(f: F) -> R
	where
		F: FnOnce(Self) -> R,
	{
		if let Some(reused) = Self::try_reuse() {
			f(reused)
		} else {
			Self::with_new_cold(f)
		}
	}

	#[cold]
	#[inline(never)]
	fn with_new_cold<F, R>(f: F) -> R
	where
		F: FnOnce(Self) -> R,
	{
		let session = MovableSessionGuard::new().make_local();
		f(session)
	}

	#[inline(always)]
	pub fn try_reuse() -> Option<Self> {
		let mut copy = LOCAL_SESSION.get();

		if let Some(ptr) = copy.ptr {
			debug_assert!(copy.rc > 0);

			// Increment RC
			copy.rc += 1;
			LOCAL_SESSION.set(copy);

			// Construct guard
			Some(LocalSessionGuard {
				_no_threading: PhantomData,
				ptr,
			})
		} else {
			None
		}
	}

	#[inline(always)]
	pub fn handle(&self) -> Session<'_> {
		Session { ptr: self.ptr }
	}
}

impl Clone for LocalSessionGuard {
	fn clone(&self) -> Self {
		Self::try_reuse().unwrap()
	}
}

impl Drop for LocalSessionGuard {
	fn drop(&mut self) {
		#[cold]
		#[inline(never)]
		fn drop_cold(ptr: Option<&'static PrimaryStorageEntry>) {
			dealloc_session(ptr.unwrap().0)
		}

		let mut copy = LOCAL_SESSION.get();

		copy.rc -= 1;

		if copy.rc == 0 {
			let ptr = copy.ptr;
			copy.ptr = None;
			LOCAL_SESSION.set(copy);

			drop_cold(ptr);
		} else {
			LOCAL_SESSION.set(copy);
		}
	}
}

// Session handle
pub struct Session<'a> {
	ptr: &'a PrimaryStorageEntry,
}

impl Session<'_> {
	pub fn slot(self) -> u8 {
		debug_assert_ne!(self.ptr.0, 0xFF);
		self.ptr.0
	}
}

impl fmt::Debug for Session<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Session").field("id", &self.slot()).finish()
	}
}

impl Copy for Session<'_> {}

impl Clone for Session<'_> {
	fn clone(&self) -> Self {
		*self
	}
}

impl hash::Hash for Session<'_> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.slot().hash(state);
	}
}

impl Eq for Session<'_> {}

impl PartialEq for Session<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.slot() == other.slot()
	}
}

// === Session Storage === //

pub struct SessionStorage<T> {
	slots: [AssertSync<T>; 256],
}

impl<T> SessionStorage<T> {
	pub const fn new(arr: [T; 256]) -> Self {
		Self {
			slots: unsafe {
				// Safety: `AssertSync` is `repr(transparent)` so the two types have the same layout.
				sizealign_checked_transmute::<[T; 256], [AssertSync<T>; 256]>(arr)
			},
		}
	}

	pub unsafe fn get_raw<'a>(&'a self, id: u8) -> &'a T {
		// Safety: provided by caller
		self.slots[id as usize].get()
	}

	#[inline(always)]
	pub fn get<'a>(&'a self, session: Session<'a>) -> &'a T {
		unsafe {
			// Safety: we know, by the fact that `session` cannot be shared between threads, that
			// we are on the only thread with access to this value.
			self.slots[session.slot() as usize].get()
		}
	}
}

pub struct LazySessionStorage<T> {
	/// The [SessionStorage] backing the lazy init storage.
	///
	/// ## Invariants
	///
	/// External users can only ever get an immutable reference to the `Option`'s contents. If the
	/// `Option` is `None`, no reference will have been given to the user.
	///
	raw: SessionStorage<UnsafeCell<Option<T>>>,
}

impl<T> LazySessionStorage<T> {
	pub const fn new() -> Self {
		Self {
			raw: SessionStorage::new(arr![UnsafeCell::new(None); 256]),
		}
	}

	#[inline(always)]
	pub fn get<'a>(&'a self, session: Session<'a>) -> Option<&'a T> {
		let option = unsafe {
			// Safety: users can only ever get an immutable reference to the contents of the option.
			self.raw.get(session).get_ref_unchecked()
		};
		option.as_ref()
	}

	#[inline(always)]
	pub fn get_or_init_using<'a, F>(&'a self, session: Session<'a>, initializer: F) -> &'a T
	where
		F: FnOnce() -> T,
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
	fn init_cold<'a, F>(&'a self, session: Session<'a>, initializer: F) -> &'a T
	where
		F: FnOnce() -> T,
	{
		// Run our initializer
		let value = initializer();

		// Ensure that our initializer has not already initialized the value.
		assert!(
			self.get(session).is_none(),
			"`initializer` cannot call `get_or_init` on its own storage."
		);

		// Initialize and return
		let slot = unsafe {
			// Safety: we know that no references to the `Option` because it is still `None` and
			// we only return references to the inner value of the `Option` if it is not `None`.
			&mut *self.raw.get(session).get()
		};

		// This cannot run a destructor to observe the mutable borrow because we already checked
		// that it was none.
		*slot = Some(value);

		// Safety: we just need to make sure to return an immutable reference now.
		slot.as_ref().unwrap()
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
	type PrimaryStorage: PrimaryStaticStorage;

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
	unsafe fn init_session(id: u8) -> &'static PrimaryStorageEntry;

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
/// **This trait is not to be implemented manually.** Instead, users should implement
/// [StaticStorageHandler] to define the actual storage's properties and then derive this trait by
/// registering the struct in the singleton call to [register_static_storages].
pub(crate) trait StaticStorage: StaticStorageHandler {
	fn get(session: Session) -> &Self::Comp;
}

// N.B. we made this `Sync`, even though `SessionStorage` doesn't need it, because we consistently
// take references to the value and these would automatically become `!Send` if the pointee were `!Sync`.
type PrimaryStorageEntryOf<T> = (u8, MutexedUnsafeCell<Option<T>>);

type PrimaryStorageEntry = PrimaryStorageEntryOf<
	<<() as StaticStorageHygieneBreak>::PrimaryStorage as StaticStorageHandler>::Comp,
>;

unsafe trait PrimaryStaticStorage: StaticStorage {
	fn name() -> &'static str;

	unsafe fn backing_storage() -> &'static SessionStorage<PrimaryStorageEntryOf<Self::Comp>>;

	unsafe fn init(id: u8) -> &'static PrimaryStorageEntryOf<Self::Comp> {
		// Safety: trust us, we're professionals. (this method is unsafe just to make sure
		// that external users keep away from our stuff)
		let storage = Self::backing_storage();

		// Safety: We're the only thread with access to this ID.
		let slot = storage.get_raw(id);

		// Initialize the state and ensure that the user hasn't messed anything up.
		let state = &mut *slot.1.get();
		Self::init_comp(state);
		assert!(
			state.is_some(),
			"`{}::init_comp` failed to initialize component.",
			Self::name(),
		);

		slot
	}

	unsafe fn deinit(id: u8) {
		// Safety: trust us, we're professionals. (this method is unsafe just to make sure
		// that external users keep away from our stuff)
		let storage = Self::backing_storage();

		// Safety: We're accessing this state after everyone else has given up access to it.
		let state = &mut *storage.get_raw(id).1.get();

		// As a gesture of kindness, we tell the compiler that the state is not `None` at this
		// point so the user can unwrap it for free.
		match state {
			Some(_) => {}
			// Safety: if this invariant didn't hold up, we'd be dead long ago.
			None => unreachable_unchecked(),
		}

		// Users can do whatever they want here.
		Self::deinit_comp(state);
	}
}

unsafe trait SecondaryStaticStorage: StaticStorage {
	fn name() -> &'static str;

	unsafe fn backing_storage() -> &'static SessionStorage<UnsafeCell<Option<Self::Comp>>>;

	unsafe fn init(id: u8) {
		// Safety: trust us, we're professionals. (this method is unsafe just to make sure
		// that external users keep away from our stuff)
		let storage = Self::backing_storage();

		// Safety: We're accessing this state before anyone else even has access to this
		// session and we release the reference before anyone else gets to read it.
		let state = &mut *storage.get_raw(id).get();

		// Initialize the state and ensure that the user hasn't messed anything up.
		Self::init_comp(state);
		assert!(
			state.is_some(),
			"`{}::init_comp` failed to initialize component.",
			Self::name(),
		);
	}

	unsafe fn deinit(id: u8) {
		// Safety: trust us, we're professionals. (this method is unsafe just to make sure
		// that external users keep away from our stuff)
		let storage = Self::backing_storage();

		// Safety: We're accessing this state after everyone else has given up access to it.
		let state = &mut *storage.get_raw(id).get();

		// As a gesture of kindness, we tell the compiler that the state is not `None` at this
		// point so the user can unwrap it for free.
		match state {
			Some(_) => {}
			// Safety: if this invariant didn't hold up, we'd be dead long ago.
			None => unreachable_unchecked(),
		}

		// Users can do whatever they want here.
		Self::deinit_comp(state);
	}
}

macro register_static_storages(
	$first_target:path
	$(,$target:path)*
	$(,)?
) {
	unsafe impl StaticStorageHygieneBreak for () {
		type PrimaryStorage = $first_target;

		unsafe fn init_session(id: u8) -> &'static PrimaryStorageEntry {
			let ptr = <$first_target as PrimaryStaticStorage>::init(id);
			$(<$target as SecondaryStaticStorage>::init(id);)*
			ptr
		}

		unsafe fn deinit_session(id: u8) {
			<$first_target as PrimaryStaticStorage>::deinit(id);
			$(<$target as SecondaryStaticStorage>::deinit(id);)*
		}
	}

	impl StaticStorage for $first_target {
		fn get<'a>(session: Session<'a>) -> &'a <Self as StaticStorageHandler>::Comp {
			unsafe {
				let entry = &session.ptr.1;
				let entry = entry.get_ref_unchecked();

				match entry {
					Some(comp) => comp,
					None => unreachable_unchecked(),
				}
			}
		}
	}

	unsafe impl PrimaryStaticStorage for $first_target {
		fn name() -> &'static str {
			stringify!($first_target)
		}

		unsafe fn backing_storage() -> &'static SessionStorage<PrimaryStorageEntry> {
			static STORAGE: SessionStorage<PrimaryStorageEntry> = {
				SessionStorage::new(arr_indexed![i => (
					i as u8,
					MutexedUnsafeCell::new(None),
				); 256])
			};
			&STORAGE
		}
	}

	$(
		impl StaticStorage for $target {
			fn get<'a>(session: Session<'a>) -> &'a <Self as StaticStorageHandler>::Comp {
				unsafe {
					let entry = Self::backing_storage().get(session);
					let entry = entry.get_ref_unchecked();

					match entry {
						Some(comp) => comp,
						None => unreachable_unchecked(),
					}
				}
			}
		}

		unsafe impl SecondaryStaticStorage for $target {
			fn name() -> &'static str {
				stringify!($target)
			}

			unsafe fn backing_storage() -> &'static SessionStorage<UnsafeCell<Option<<Self as StaticStorageHandler>::Comp>>> {
				static STORAGE: SessionStorage<UnsafeCell<Option<<$target as StaticStorageHandler>::Comp>>> =
					SessionStorage::new(arr![UnsafeCell::new(None); 256]);
				&STORAGE
			}
		}
	)*
}

// TODO: Use AOS instead of SOA; we're not iterating through any of these lists.
register_static_storages![
	super::lock::SessionLockState,
	super::object_db::SessionSlotManagerState
];
