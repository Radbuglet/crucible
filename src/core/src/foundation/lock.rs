use self::internals::{wake_up_requests, LockRequestHandle, LockRequestState};

use crate::util::bitmask::Bitmask64;
use crate::util::error::ResultExt;
use crate::util::tuple::impl_tuples;
use log::trace;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

// === Lock management === //

#[doc(hidden)]
pub use self::internals::RwLockManager;

#[doc(hidden)]
pub use self::internals::RwMask;

mod internals {
	use crate::foundation::event::EventPusher;
	use crate::util::bitmask::Bitmask64;
	use crate::util::meta_enum::{enum_meta, EnumMeta};
	use log::trace;
	use std::cell::UnsafeCell;
	use std::collections::VecDeque;
	use std::ops::{BitOr, BitOrAssign, Deref, DerefMut};
	use std::sync::atomic::{AtomicU8, Ordering};
	use std::sync::{Arc, Mutex};
	use std::task::Waker;

	/// Manages lock states for up to 64 [RwLock](super::RwLock)s. [RwLockManager]s can be cloned and
	/// shared across threads. It is not possible to acquire locks from two different managers atomically.
	#[derive(Default, Clone)]
	pub struct RwLockManager {
		inner: Arc<Mutex<LockManagerInner>>,
	}

	impl RwLockManager {
		/// Creates a new [RwLockManager].
		pub fn new() -> Self {
			Default::default()
		}

		/// Acquires exclusive access to the [RwLockManager]'s inner state. Control should never be
		/// returned to user-controlled logic while the returned mutex guard is alive since users
		/// could easily cause a deadlock. As with everything mutexes, this function may block.
		pub(super) fn inner(&self) -> impl Deref<Target = LockManagerInner> + DerefMut + '_ {
			self.inner
				.lock()
				.expect("internal RwLockManager poisoning error")
		}
	}

	impl Eq for RwLockManager {}
	impl PartialEq for RwLockManager {
		fn eq(&self, other: &Self) -> bool {
			Arc::ptr_eq(&self.inner, &other.inner)
		}
	}

	/// [RwLockManager]'s internal state, keeping authoritative records of which locks are allocated,
	/// their acquisition states, and atomic requests for them.
	pub struct LockManagerInner {
		/// A mask of all locks that are currently reserved. Used to quickly allocate new lock indices.
		reserved_locks: Bitmask64,

		/// Maps lock indices to lock state. Negative values indicate shared immutable references, `0`
		/// indicates a free lock, and `1` indicates an exclusive mutable lock.
		locks: [isize; 64],

		/// The mask of which features of every lock can be acquired. Updated alongside `locks`.
		available_locks: RwMask,

		/// An unordered set of requests.
		// Ideally, futures would have ownership over "LockPendingState" instances to avoid allocations
		// but this is a really small micro-optimization that seems pretty hard to implement correctly
		// so I'm going to hold off on it until later.
		requests: Vec<LockRequest>,
	}

	struct LockRequest {
		deps: RwMask,
		state: Arc<LockRequestHandle>,
	}

	impl Default for LockManagerInner {
		fn default() -> Self {
			Self {
				reserved_locks: Bitmask64(0),
				locks: [0; 64],
				available_locks: RwMask::FULL,
				requests: Vec::new(),
			}
		}
	}

	impl LockManagerInner {
		// === Lock management === //

		/// Allocates a new lock.
		pub fn new_lock(&mut self) -> usize {
			// Reserve index
			let index = self
				.reserved_locks
				.reserve_flag()
				.expect("Cannot allocate more than 64 locks on a pool!");

			// Reset counter
			self.locks[index] = 0;

			// Reset masks
			let mask = Bitmask64::one_hot(index);
			self.available_locks.read.add(mask);
			self.available_locks.write.add(mask);

			trace!(
				"Manager {:p}: created a new lock with index {}",
				self,
				index
			);

			index
		}

		/// Destroys a lock immediately, cancelling all requesting involving that lock. We push
		/// requests to wake up the relevant requests to the provided [EventPusher] so that user code
		/// can be invoked after the [LockManagerInner] mutex has been released so as to avoid
		/// deadlocks in the futures.
		pub fn del_lock<'a>(
			&mut self,
			lock: usize,
			ev_wakeup: &mut impl EventPusher<'a, Event = Arc<LockRequestHandle>>,
		) {
			trace!("Manager {:p}: destroying lock with index {}", self, lock);

			// Unregister lock
			self.reserved_locks.remove(Bitmask64::one_hot(lock));

			// Cancel dependent requests
			let removed = Bitmask64::one_hot(lock);
			self.poll_locks_common(
				&|req| {
					req.deps.read.is_superset_of(removed) || req.deps.write.is_superset_of(removed)
				},
				RemoveWhereMode::Destroy,
				ev_wakeup,
			);
		}

		// === Lock updating === //

		/// Returns whether we can acquire an entire set of locks atomically.
		pub fn can_lock_mask(&self, mask: RwMask) -> bool {
			debug_assert!(mask.is_valid());
			self.available_locks.is_superset_of(&mask)
		}

		/// Acquires a set of locks atomically, panicking in debug builds if any of the locks cannot
		/// be acquired.
		pub fn lock_mask(&mut self, mask: RwMask) {
			debug_assert!(self.can_lock_mask(mask));

			// Update mask
			self.available_locks.write.remove(mask.read); // No write if read
			self.available_locks.write.remove(mask.write);
			self.available_locks.read.remove(mask.write); // No read if write

			// Update counters
			for lock in mask.read.iter_ones() {
				trace!("Manager {:p}: immutably locking {}", self, lock);
				debug_assert!(self.locks[lock] <= 0);

				// This is not checked by the lock mask but is rare enough to not have to.
				assert_ne!(self.locks[lock], isize::MIN);
				self.locks[lock] -= 1;
			}

			for lock in mask.write.iter_ones() {
				trace!("Manager {:p}: mutably locking {}", self, lock);
				debug_assert!(self.locks[lock] == 0);
				self.locks[lock] = 1;
			}
		}

		/// Acquires a set of locks atomically, returning `false` if any of the locks cannot be
		/// acquired without acquiring the locks which can be acquired.
		pub fn try_lock_mask(&mut self, mask: RwMask) -> bool {
			if self.can_lock_mask(mask) {
				self.lock_mask(mask);
				true
			} else {
				false
			}
		}

		/// Releases a set of locks atomically. There is no difference (beyond performance) between
		/// releasing each acquired lock in one call versus locking them in several separate calls.
		/// Users should call [poll_completed] once they are done unlocking locks so that blocked
		/// requests can complete.
		pub fn unlock_mask(&mut self, mask: RwMask) {
			// Unlock read locks
			// We have to update each lock mask entry independently because the mask state depends
			// on the lock count.
			for lock in mask.read.iter_ones() {
				trace!("Manager {:p}: immutably unlocking {}", self, lock);
				debug_assert!(self.locks[lock] < 0);
				self.locks[lock] += 1;
				if self.locks[lock] == 0 {
					self.available_locks.write.add(Bitmask64::one_hot(lock));
				}
			}

			// Update mutable lock counters
			for lock in mask.write.iter_ones() {
				trace!("Manager {:p}: mutably unlocking {}", self, lock);
				debug_assert!(self.locks[lock] == 1);
				self.locks[lock] = 0;
			}

			// Update mutable lock masks
			self.available_locks.write.add(mask.write); // Exclusive access relinquished. We're now at 0.
			self.available_locks.read.add(mask.write);
		}

		// === Request tracking & polling === //

		/// Registers a new atomic lock request into the unordered queue. The request will wait until
		/// all dependent locks can be acquired at once. The request will only progress as [poll_completed]
		/// gets called, meaning that users should try to lock the dependencies immediately before
		/// registering a request. The request will terminate with a [Destroyed](LockRequestState::Destroyed)
		/// state if any of the dependency locks are destroyed.
		pub fn add_request(
			&mut self,
			dependencies: RwMask,
			waker: Waker,
		) -> Arc<LockRequestHandle> {
			debug_assert!(dependencies.is_valid());

			let state = Arc::new(LockRequestHandle::new(waker, self.requests.len()));
			self.requests.push(LockRequest {
				deps: dependencies,
				state: state.clone(),
			});

			trace!(
				"Manager {:p}: created new request for {:?} with handle {:p}",
				self,
				dependencies,
				state
			);

			state
		}

		/// Cancels a request without waking it up or updating its state.
		pub unsafe fn forget_request(&mut self, request: &LockRequestHandle) {
			trace!(
				"Manager {:p}: forgetting request with handle {:p}",
				self,
				request
			);
			self.remove_request(request.get_index());
		}

		/// Polls for locks requests that can be atomically acquired, marking them as [Available](LockRequestState::Available)
		/// and removing them from the queue. Completed lock requests are pushed to the provided
		/// [EventPusher] so that external code can invoke wakers after the [LockManagerInner] mutex
		/// has been released so as to avoid deadlocks within the futures.
		pub fn poll_completed<'a>(
			&mut self,
			ev_wakeup: &mut impl EventPusher<'a, Event = Arc<LockRequestHandle>>,
		) {
			let available_locks = self.available_locks;
			self.poll_locks_common(
				&|request| available_locks.is_superset_of(&request.deps),
				RemoveWhereMode::Finish,
				ev_wakeup,
			)
		}

		// === Internal utils === //

		fn poll_locks_common<'a, F: Fn(&LockRequest) -> bool>(
			&mut self,
			is_applicable: &F,
			mode: RemoveWhereMode,
			on_removed: &mut impl EventPusher<'a, Event = Arc<LockRequestHandle>>,
		) {
			trace!(
				"Manager {:p}: polling for requests under mode {:?}",
				self,
				mode
			);

			let mut index = 0;
			while index < self.requests.len() {
				let request = &self.requests[index];
				if is_applicable(request) {
					trace!("Manager {:p}: finalized request {:p}", self, request);

					// Update the lock's polling state appropriately.
					request.state.update_state(*mode.meta());

					// Update locks if we're supposed to apply them.
					if mode == RemoveWhereMode::Finish {
						let deps = request.deps;
						self.lock_mask(deps);
					}

					// Notify the removal event, which will probably queue up a waker dispatch.
					on_removed.push(self.remove_request(index).state);
				} else {
					index += 1;
				}
			}

			trace!("Manager {:p}: done polling", self);
		}

		fn remove_request(&mut self, index: usize) -> LockRequest {
			let removed = self.requests.swap_remove(index);
			if let Some(moved_req) = self.requests.get(index) {
				unsafe { moved_req.state.update_index(index) };
			}
			removed
		}
	}

	enum_meta! {
		#[derive(Debug)]
		enum(LockRequestState) RemoveWhereMode {
			Finish = LockRequestState::Available,
			Destroy = LockRequestState::Destroyed,
		}
	}

	/// A handle for a request. The handle must be unregistered manually by calling
	/// [RwLockManagerInner::forget_request] on the owning manager.
	pub struct LockRequestHandle {
		// "index" is externally synchronized with "LockManagerInner".
		index: UnsafeCell<usize>,
		// We have to use atomic operations here because the future may be polled while the manager is
		// updating the states.
		state: AtomicU8,
		waker: Waker,
	}

	unsafe impl Sync for LockRequestHandle {}

	impl LockRequestHandle {
		fn new(waker: Waker, index: usize) -> Self {
			Self {
				index: UnsafeCell::new(index),
				state: AtomicU8::new(LockRequestState::Pending.index() as u8),
				waker,
			}
		}

		unsafe fn update_index(&self, index: usize) {
			*self.index.get() = index;
		}

		unsafe fn get_index(&self) -> usize {
			*self.index.get()
		}

		fn update_state(&self, state: LockRequestState) {
			// We don't care about flushing the `RwLockManager`'s state changes since they're already
			// made visible to the consuming thread by the mutex.
			self.state.store(state.index() as u8, Ordering::Relaxed);
		}

		pub fn state(&self) -> LockRequestState {
			LockRequestState::from_index(self.state.load(Ordering::Relaxed) as usize)
		}

		pub fn waker(&self) -> &Waker {
			&self.waker
		}
	}

	enum_meta! {
		/// The state of a lock request.
		#[derive(Debug)]
		pub enum(u8) LockRequestState {
			/// The lock request has completed successfully.
			Available = 0,

			/// One or more locks the request depended upon have been destroyed, rendering the request
			/// impossible to fulfill.
			Destroyed = 1,

			/// The request is still waiting for dependency locks to become available.
			Pending = 2,
		}
	}

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub struct RwMask {
		pub read: Bitmask64,
		pub write: Bitmask64,
	}

	impl RwMask {
		pub const EMPTY: Self = Self {
			read: Bitmask64::EMPTY,
			write: Bitmask64::EMPTY,
		};

		pub const FULL: Self = Self {
			read: Bitmask64::FULL,
			write: Bitmask64::FULL,
		};

		pub fn is_superset_of(&self, other: &Self) -> bool {
			self.read.is_superset_of(other.read) && self.write.is_superset_of(other.write)
		}

		pub fn is_valid(&self) -> bool {
			(self.read & self.write).is_empty()
		}

		pub fn checked_merge<I: IntoIterator<Item = Self>>(iter: I) -> Option<Self> {
			let mut accum = Self::EMPTY;

			for comp in iter {
				// Ensure that we don't mutably borrow the same lock twice
				if accum.write.contains(comp.write) {
					return None;
				}

				accum |= comp;
			}

			// Ensure that the resulting mask doesn't mutably and immutably borrow a lock
			// simultaneously.
			if !accum.is_valid() {
				return None;
			}

			Some(accum)
		}
	}

	impl BitOr for RwMask {
		type Output = Self;

		fn bitor(self, rhs: Self) -> Self::Output {
			Self {
				read: self.read | rhs.read,
				write: self.write | rhs.write,
			}
		}
	}

	impl BitOrAssign for RwMask {
		fn bitor_assign(&mut self, rhs: Self) {
			self.read |= rhs.read;
			self.write |= rhs.write;
		}
	}

	pub fn wake_up_requests(requests: &mut VecDeque<Arc<LockRequestHandle>>) {
		for request in requests.drain(..) {
			request.waker().wake_by_ref();
		}
	}
}

// === Lock targets === //

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum MaskBuildError {
	RwAliasing,
	NonUniqueManager,
}

impl Error for MaskBuildError {}

impl Display for MaskBuildError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			MaskBuildError::RwAliasing => write!(
				f,
				"cannot acquire a lock both mutably and immutably in the same target"
			),
			MaskBuildError::NonUniqueManager => {
				write!(f, "lock targets must share the same RwLockManager")
			}
		}
	}
}

#[doc(hidden)]
pub unsafe trait LockTarget {
	type Deps: Clone;

	unsafe fn get(deps: &Self::Deps) -> Self;
	fn manager(deps: &Self::Deps) -> &RwLockManager;
	fn mask(deps: &Self::Deps) -> Result<RwMask, MaskBuildError>;
}

unsafe impl<'a, T: ?Sized> LockTarget for &'a mut T {
	type Deps = &'a RwLock<T>;

	unsafe fn get(deps: &Self::Deps) -> Self {
		&mut *(deps.value.get())
	}

	fn manager(deps: &Self::Deps) -> &RwLockManager {
		&deps.manager
	}

	fn mask(deps: &Self::Deps) -> Result<RwMask, MaskBuildError> {
		// No need to validate this mask. It is guaranteed to be valid.
		Ok(RwMask {
			read: Bitmask64::EMPTY,
			write: Bitmask64::one_hot(deps.index),
		})
	}
}

unsafe impl<'a, T: ?Sized> LockTarget for &'a T {
	type Deps = &'a RwLock<T>;

	unsafe fn get(deps: &Self::Deps) -> Self {
		&*(deps.value.get())
	}

	fn manager(deps: &Self::Deps) -> &RwLockManager {
		&deps.manager
	}

	fn mask(deps: &Self::Deps) -> Result<RwMask, MaskBuildError> {
		// No need to validate this mask. It is guaranteed to be valid.
		Ok(RwMask {
			read: Bitmask64::one_hot(deps.index),
			write: Bitmask64::EMPTY,
		})
	}
}

macro impl_lock_target_tup($($ty:ident:$field:tt),*) {
	unsafe impl<'a, $($ty: LockTarget),*> LockTarget for ($($ty,)*) {
		type Deps = ($($ty::Deps,)*);

		unsafe fn get(deps: &Self::Deps) -> Self {
			($($ty::get(&deps.$field),)*)
		}

		#[allow(unreachable_code)]  // We do this intentionally.
		fn manager(deps: &Self::Deps) -> &RwLockManager {
			$(return $ty::manager(&deps.$field);)*  // Return the first manager
		}

		fn mask(deps: &Self::Deps) -> Result<RwMask, MaskBuildError> {
			// Validate managers
			let manager = Self::manager(deps);

			if $(manager != $ty::manager(&deps.$field) ||)* false {
				return Err(MaskBuildError::NonUniqueManager);
			}

			// Build mask, checking if it is valid.
			RwMask::checked_merge([$($ty::mask(&deps.$field)?,)*].iter().copied())
				.ok_or(MaskBuildError::RwAliasing)
		}
	}
}

impl_tuples!(no_unit; impl_lock_target_tup);

// === RwLock === //

pub struct RwLock<T: ?Sized> {
	manager: RwLockManager,
	index: usize,
	value: UnsafeCell<T>,
}

unsafe impl<T: ?Sized> Send for RwLock<T> {}
unsafe impl<T: ?Sized> Sync for RwLock<T> {}

impl<T> RwLock<T> {
	pub fn new<M: Into<RwLockManager>>(manager: M, value: T) -> Self {
		let manager = manager.into();
		let index = manager.inner().new_lock();
		Self {
			manager,
			index,
			value: UnsafeCell::new(value),
		}
	}
}

impl<T: ?Sized> RwLock<T> {
	// === Immediate locking === //

	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}

	pub fn try_lock_mut_now(&self) -> Option<RwGuardMut<T>> {
		RwGuard::try_lock_now(self)
	}

	pub fn try_lock_ref_now(&self) -> Option<RwGuardRef<T>> {
		RwGuard::try_lock_now(self)
	}

	pub fn lock_mut_now(&self) -> RwGuardMut<T> {
		self.try_lock_mut_now().unwrap()
	}

	pub fn lock_ref_now(&self) -> RwGuardRef<T> {
		self.try_lock_ref_now().unwrap()
	}

	// === Async locking === //

	pub fn lock_mut_async_or_fail(&self) -> RwLockFuture<&mut T> {
		RwLockFuture::new(self)
	}

	pub async fn lock_mut_async(&self) -> RwGuardMut<'_, T> {
		self.lock_mut_async_or_fail().await.unwrap_pretty()
	}

	pub fn lock_ref_async_or_fail(&self) -> RwLockFuture<&T> {
		RwLockFuture::new(self)
	}

	pub async fn lock_ref_async(&self) -> RwGuardRef<'_, T> {
		self.lock_ref_async_or_fail().await.unwrap_pretty()
	}
}

impl<T: ?Sized> Drop for RwLock<T> {
	fn drop(&mut self) {
		let mut wakeup = VecDeque::new();
		self.manager.inner().del_lock(self.index, &mut wakeup);
		wake_up_requests(&mut wakeup);
	}
}

// === RwLockFuture === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RwAsyncLockError {
	Destroyed,
}

impl Error for RwAsyncLockError {}

impl Display for RwAsyncLockError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Destroyed => write!(f, "Dependency lock destroyed."),
		}
	}
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct RwLockFuture<T: LockTarget> {
	state: FutureState,
	deps: T::Deps,
}

enum FutureState {
	Idle,
	Waiting {
		mask: RwMask,
		handle: Arc<LockRequestHandle>,
	},
	Done,
}

impl<T: LockTarget> Unpin for RwLockFuture<T> {}

impl<T: LockTarget> RwLockFuture<T> {
	pub fn new(deps: T::Deps) -> Self {
		Self {
			state: FutureState::Idle,
			deps,
		}
	}
}

impl<T: LockTarget> Future for RwLockFuture<T> {
	type Output = Result<RwGuard<T>, RwAsyncLockError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match &self.state {
			FutureState::Idle => {
				let mut manager = T::manager(&self.deps).inner();
				let mask = T::mask(&self.deps).unwrap_pretty();

				if manager.try_lock_mask(mask) {
					drop(manager);
					self.state = FutureState::Done;

					trace!("RwLockFuture finished immediately from idle!");
					Poll::Ready(Ok(RwGuard {
						mask,
						deps: self.deps.clone(),
					}))
				} else {
					trace!("RwLockFuture creating request from idle...");

					let handle = manager.add_request(mask, cx.waker().clone());
					drop(manager);
					self.state = FutureState::Waiting { mask, handle };
					Poll::Pending
				}
			}
			FutureState::Waiting { mask, handle } => match handle.state() {
				LockRequestState::Pending => {
					trace!("RwLockFuture is still pending...");
					Poll::Pending
				}
				LockRequestState::Available => {
					let mask = *mask;
					self.state = FutureState::Done;
					trace!("RwLockFuture resolving asynchronously with success!");
					Poll::Ready(Ok(RwGuard {
						mask,
						deps: self.deps.clone(),
					}))
				}
				LockRequestState::Destroyed => {
					self.state = FutureState::Done;
					trace!("RwLockFuture resolving asynchronously with error!");
					Poll::Ready(Err(RwAsyncLockError::Destroyed))
				}
			},
			FutureState::Done => {
				panic!("Cannot poll a future after it's done!");
			}
		}
	}
}

impl<T: LockTarget> Drop for RwLockFuture<T> {
	fn drop(&mut self) {
		if let FutureState::Waiting { handle, .. } = &self.state {
			let mut manager = T::manager(&self.deps).inner();
			unsafe { manager.forget_request(handle) };
		}
	}
}

// === RwGuard === //

pub type RwGuardMut<'a, T> = RwGuard<&'a mut T>;
pub type RwGuardRef<'a, T> = RwGuard<&'a T>;

pub struct RwGuard<T: LockTarget> {
	mask: RwMask,
	deps: T::Deps,
}

impl<T: LockTarget> RwGuard<T> {
	// === Guard constructors === //

	pub fn try_lock_now(deps: T::Deps) -> Option<Self> {
		let mask = T::mask(&deps).unwrap_pretty();
		if T::manager(&deps).inner().try_lock_mask(mask) {
			Some(Self { mask, deps })
		} else {
			None
		}
	}

	pub fn lock_now(targets: T::Deps) -> Self {
		Self::try_lock_now(targets).unwrap()
	}

	pub fn lock_async_or_fail(targets: T::Deps) -> RwLockFuture<T> {
		RwLockFuture::new(targets)
	}

	pub async fn lock_async(targets: T::Deps) -> Self {
		Self::lock_async_or_fail(targets).await.unwrap_pretty()
	}

	// === Fetching === //

	pub fn get(&self) -> T {
		unsafe { T::get(&self.deps) }
	}
}

impl<T: LockTarget> Drop for RwGuard<T> {
	fn drop(&mut self) {
		let mut manager = T::manager(&self.deps).inner();

		// Release acquired locks
		manager.unlock_mask(self.mask);

		// Poll for new lock requests
		let mut wakeup = VecDeque::new();
		manager.poll_completed(&mut wakeup);
		wake_up_requests(&mut wakeup);
	}
}

pub macro lock_many_now($target:expr => $guard:ident, $($comp:ident : $ty:ty),+$(,)?) {
	let $guard = RwGuard::<($($ty,)*)>::lock_now($target);
	let ($($comp,)*) = $guard.get();
}
