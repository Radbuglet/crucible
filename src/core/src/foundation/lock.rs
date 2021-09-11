use self::internals::RwMask;
use crate::foundation::event::EventPusherPoll;
use crate::foundation::lock::internals::{LockRequestHandle, LockRequestState};
use crate::util::bitmask::Bitmask64;
use crate::util::tuple::impl_tuples;
use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

// === Core lock management === //

pub use self::internals::RwLockManager;

pub mod internals {
	use crate::foundation::event::EventPusher;
	use crate::util::bitmask::Bitmask64;
	use std::cell::UnsafeCell;
	use std::ops::{BitAnd, BitAndAssign, Deref, DerefMut};
	use std::sync::atomic::{AtomicU8, Ordering};
	use std::sync::{Arc, Mutex};
	use std::task::Waker;

	#[derive(Default, Clone)]
	pub struct RwLockManager {
		inner: Arc<Mutex<LockManagerInner>>,
	}

	impl RwLockManager {
		pub fn new() -> Self {
			Default::default()
		}

		pub fn inner(&self) -> impl Deref<Target = LockManagerInner> + DerefMut + '_ {
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

	pub struct LockManagerInner {
		reserved_locks: Bitmask64,
		locks: [isize; 64],
		available_locks: RwMask,
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

		pub fn new_lock(&mut self) -> usize {
			let index = self
				.reserved_locks
				.reserve_flag()
				.expect("Cannot allocate more than 64 locks on a pool!");

			self.locks[index] = 0;
			index
		}

		pub fn del_lock(
			&mut self,
			lock: usize,
			ev_wakeup: &mut impl EventPusher<Event = Arc<LockRequestHandle>>,
		) {
			self.reserved_locks.remove(Bitmask64::one_hot(lock));
			let removed = Bitmask64::one_hot(lock);
			self.poll_locks_common(
				&|req| req.deps.read.contains(&removed) || req.deps.write.contains(&removed),
				RemoveWhereMode::Destroy,
				ev_wakeup,
			);
		}

		// === Lock updating === //

		pub fn can_lock_mut(&self, lock: usize) -> bool {
			self.locks[lock] == 0
		}

		fn lock_mut_counter(&mut self, lock: usize) {
			debug_assert!(self.can_lock_mut(lock));
			self.locks[lock] = 1;
		}

		pub fn lock_mut(&mut self, lock: usize) {
			// Update the counter w/ debug checks
			self.lock_mut_counter(lock);

			// Update RW masks
			let mask = Bitmask64::one_hot(lock);
			self.available_locks.read.remove(mask);
			self.available_locks.write.remove(mask);
		}

		pub fn try_lock_mut(&mut self, lock: usize) -> bool {
			if self.can_lock_mut(lock) {
				self.lock_mut(lock);
				true
			} else {
				false
			}
		}

		pub fn can_lock_ref(&self, lock: usize) -> bool {
			self.locks[lock] <= 0
		}

		fn lock_ref_counter(&mut self, lock: usize) {
			debug_assert!(self.can_lock_ref(lock));
			assert_ne!(self.locks[lock], isize::MIN);
			self.locks[lock] -= 1;
		}

		pub fn lock_ref(&mut self, lock: usize) {
			// Update the counter w/ debug checks
			self.lock_ref_counter(lock);

			// Update RW masks
			self.available_locks.write.remove(Bitmask64::one_hot(lock));
		}

		pub fn try_lock_ref(&mut self, lock: usize) -> bool {
			if self.can_lock_ref(lock) {
				self.lock_ref(lock);
				true
			} else {
				false
			}
		}

		fn unlock_mut_counter(&mut self, lock: usize) {
			debug_assert!(self.locks[lock] == 1);
			self.locks[lock] = 0;
		}

		pub fn unlock_mut(&mut self, lock: usize) {
			// Update the counter w/ debug checks
			self.unlock_mut_counter(lock);

			// Update RW masks
			let mask = Bitmask64::one_hot(lock);
			self.available_locks.read.add(mask);
			self.available_locks.write.add(mask);
		}

		pub fn unlock_ref(&mut self, lock: usize) {
			debug_assert!(self.locks[lock] < 0);
			self.locks[lock] += 1;
			if self.locks[lock] == 0 {
				self.available_locks.write.add(Bitmask64::one_hot(lock));
			}
		}

		pub fn can_lock_mask(&self, mask: RwMask) -> bool {
			self.available_locks.contains(&mask)
		}

		pub fn lock_mask(&mut self, mask: RwMask) {
			debug_assert!(self.can_lock_mask(mask));

			// Update mask
			self.available_locks.write.remove(mask.read); // No write if read
			self.available_locks.write.remove(mask.write);
			self.available_locks.read.remove(mask.write); // No read if write

			// Update counters
			for lock in mask.read.iter_ones() {
				self.lock_ref_counter(lock);
			}

			for lock in mask.write.iter_ones() {
				self.lock_mut_counter(lock);
			}
		}

		pub fn try_lock_mask(&mut self, mask: RwMask) -> bool {
			if self.can_lock_mask(mask) {
				self.lock_mask(mask);
				true
			} else {
				false
			}
		}

		pub fn unlock_mask(&mut self, mask: RwMask) {
			// Unlock read locks
			// We have to update each lock mask entry independently because the mask state depends
			// on the lock count.
			for lock in mask.read.iter_ones() {
				self.unlock_ref(lock);
			}

			// Update mutable lock masks
			self.available_locks.write.add(mask.write); // Exclusive access relinquished. We're now at 0.
			self.available_locks.read.add(mask.write);

			// Update mutable lock counters
			for lock in mask.write.iter_ones() {
				self.unlock_mut_counter(lock);
			}
		}

		// === Request tracking & polling === //

		pub fn add_request(
			&mut self,
			dependencies: RwMask,
			waker: Waker,
		) -> Arc<LockRequestHandle> {
			let state = Arc::new(LockRequestHandle::new(waker, self.requests.len()));
			self.requests.push(LockRequest {
				deps: dependencies,
				state: state.clone(),
			});
			state
		}

		pub unsafe fn forget_request(&mut self, request: &LockRequestHandle) {
			self.remove_request(request.get_index());
		}

		pub fn poll_completed(
			&mut self,
			ev_wakeup: &mut impl EventPusher<Event = Arc<LockRequestHandle>>,
		) {
			let available_locks = self.available_locks;
			self.poll_locks_common(
				&|request| available_locks.contains(&request.deps),
				RemoveWhereMode::Finish,
				ev_wakeup,
			)
		}

		// === Internal utils === //

		fn poll_locks_common<F: Fn(&LockRequest) -> bool>(
			&mut self,
			is_applicable: &F,
			mode: RemoveWhereMode,
			on_removed: &mut impl EventPusher<Event = Arc<LockRequestHandle>>,
		) {
			let mut index = 0;
			while index < self.requests.len() {
				let request = &self.requests[index];
				if is_applicable(request) {
					// Update the lock's polling state appropriately.
					request.state.update_state(match mode {
						RemoveWhereMode::Finish => LockRequestState::Available,
						RemoveWhereMode::Destroy => LockRequestState::Destroyed,
					});

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
		}

		fn remove_request(&mut self, index: usize) -> LockRequest {
			let removed = self.requests.swap_remove(index);
			if let Some(moved_req) = self.requests.get(index) {
				unsafe { moved_req.state.update_index(index) };
			}
			removed
		}
	}

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	enum RemoveWhereMode {
		Finish = 0,
		Destroy = 1,
	}

	pub struct LockRequestHandle {
		// "index" is internally synchronized with "LockManagerInner".
		index: UnsafeCell<usize>,
		// We have to use atomic operations here because the future may be polled while the manager is
		// updating the states.
		state: AtomicU8,
		waker: Waker,
	}

	impl LockRequestHandle {
		fn new(waker: Waker, index: usize) -> Self {
			Self {
				index: UnsafeCell::new(index),
				state: AtomicU8::new(LockRequestState::Pending as u8),
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
			// TODO: Is this ordering sufficient to make the changes visible to the future?
			self.state.store(state as u8, Ordering::Relaxed);
		}

		pub fn state(&self) -> LockRequestState {
			match self.state.load(Ordering::Relaxed) {
				0 => LockRequestState::Pending,
				1 => LockRequestState::Available,
				2 => LockRequestState::Destroyed,
				_ => panic!("Unknown PendingState discriminant!"),
			}
		}

		pub fn waker(&self) -> &Waker {
			&self.waker
		}
	}

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub enum LockRequestState {
		Available = 0,
		Destroyed = 1,
		Pending = 2,
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

		pub fn contains(&self, other: &Self) -> bool {
			self.read.contains(&other.read) && self.write.contains(&other.write)
		}
	}

	impl BitAnd for RwMask {
		type Output = Self;

		fn bitand(self, rhs: Self) -> Self::Output {
			Self {
				read: self.read & rhs.read,
				write: self.write & rhs.write,
			}
		}
	}

	impl BitAndAssign for RwMask {
		fn bitand_assign(&mut self, rhs: Self) {
			self.read &= rhs.read;
			self.write &= rhs.write;
		}
	}
}

// === Lock targets === //

pub unsafe trait LockTarget: Clone {
	fn validate(&self);
	fn manager(&self) -> &RwLockManager;
	fn mask(&self) -> RwMask;
}

pub struct RwRef<'a, T: ?Sized>(pub &'a RwLock<T>);

impl<T: ?Sized> Copy for RwRef<'_, T> {}
impl<T: ?Sized> Clone for RwRef<'_, T> {
	fn clone(&self) -> Self {
		Self(self.0)
	}
}

unsafe impl<'a, T: ?Sized> LockTarget for RwRef<'a, T> {
	fn validate(&self) {
		// No-op: single locks are always valid.
	}

	fn manager(&self) -> &RwLockManager {
		&self.0.manager
	}

	fn mask(&self) -> RwMask {
		RwMask {
			read: Bitmask64::one_hot(self.0.index),
			write: Bitmask64::EMPTY,
		}
	}
}

pub struct RwMut<'a, T: ?Sized>(pub &'a RwLock<T>);

impl<T: ?Sized> Copy for RwMut<'_, T> {}
impl<T: ?Sized> Clone for RwMut<'_, T> {
	fn clone(&self) -> Self {
		Self(self.0)
	}
}

unsafe impl<'a, T: ?Sized> LockTarget for RwMut<'a, T> {
	fn validate(&self) {
		// No-op: single locks are always valid.
	}

	fn manager(&self) -> &RwLockManager {
		&self.0.manager
	}

	fn mask(&self) -> RwMask {
		RwMask {
			read: Bitmask64::EMPTY,
			write: Bitmask64::one_hot(self.0.index),
		}
	}
}

macro impl_lock_target_tup($($ty:ident:$field:tt),*) {
	unsafe impl<'a, $($ty: LockTarget),*> LockTarget for ($($ty,)*) {
		fn validate(&self) {
			if $(self.0.manager() != self.$field.manager() ||)* false {
				panic!("Locks within an atomic lock guard must share the same manager!");
			}

			// FIXME: Check for local lock collisions (this impacts soundness!!)
		}

		fn manager(&self) -> &RwLockManager {
			self.0.manager()
		}

		fn mask(&self) -> RwMask {
			$(self.$field.mask() & )* RwMask::EMPTY
		}
	}
}

impl_tuples!(no_unit; impl_lock_target_tup);

// === Public lock API === //

pub struct RwLock<T: ?Sized> {
	manager: RwLockManager,
	index: usize,
	value: UnsafeCell<T>,
}

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
	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}

	pub fn try_lock_mut_now(&self) -> Option<RwGuardMut<T>> {
		RwGuard::try_lock_now(RwMut(self))
	}

	pub fn try_lock_ref_now(&self) -> Option<RwGuardRef<T>> {
		RwGuard::try_lock_now(RwRef(self))
	}

	pub fn lock_mut_now(&self) -> RwGuardMut<T> {
		self.try_lock_mut_now().unwrap()
	}

	pub fn lock_ref_now(&self) -> RwGuardRef<T> {
		self.try_lock_ref_now().unwrap()
	}
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct RwLockFuture<T: LockTarget> {
	state: FutureState,
	targets: T,
}

enum FutureState {
	Idle,
	Waiting {
		mask: RwMask,
		handle: Arc<LockRequestHandle>,
	},
	Done,
}

impl<T: LockTarget> RwLockFuture<T> {
	pub fn new(targets: T) -> Self {
		targets.validate();

		Self {
			state: FutureState::Idle,
			targets,
		}
	}
}

impl<T: LockTarget> Unpin for RwLockFuture<T> {}

impl<T: LockTarget> Future for RwLockFuture<T> {
	type Output = Result<RwGuard<T>, ()>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match &self.state {
			FutureState::Idle => {
				let mut manager = self.targets.manager().inner();
				let mask = self.targets.mask();
				if manager.try_lock_mask(mask) {
					drop(manager);
					self.state = FutureState::Done;

					Poll::Ready(Ok(RwGuard {
						mask,
						targets: self.targets.clone(),
					}))
				} else {
					let handle = manager.add_request(mask, cx.waker().clone());
					drop(manager);
					self.state = FutureState::Waiting { mask, handle };
					Poll::Pending
				}
			}
			FutureState::Waiting { mask, handle } => match handle.state() {
				LockRequestState::Pending => Poll::Pending,
				LockRequestState::Available => {
					let mask = *mask;
					self.state = FutureState::Done;

					Poll::Ready(Ok(RwGuard {
						mask,
						targets: self.targets.clone(),
					}))
				}
				LockRequestState::Destroyed => {
					self.state = FutureState::Done;

					Poll::Ready(Err(()))
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
			let mut manager = self.targets.manager().inner();
			unsafe { manager.forget_request(handle) };
		}
	}
}

pub type RwGuardMut<'a, T> = RwGuard<RwMut<'a, T>>;
pub type RwGuardRef<'a, T> = RwGuard<RwRef<'a, T>>;

pub struct RwGuard<T: LockTarget> {
	mask: RwMask,
	targets: T,
}

impl<T: LockTarget> RwGuard<T> {
	pub fn try_lock_now(targets: T) -> Option<Self> {
		targets.validate();

		let mask = targets.mask();
		if targets.manager().inner().try_lock_mask(mask) {
			Some(Self { mask, targets })
		} else {
			None
		}
	}

	pub fn lock_now(targets: T) -> Self {
		Self::try_lock_now(targets).unwrap()
	}
}

impl<T: LockTarget> Drop for RwGuard<T> {
	fn drop(&mut self) {
		let mut manager = self.targets.manager().inner();
		manager.unlock_mask(self.mask);

		let mut wakeup = EventPusherPoll::new();
		manager.poll_completed(&mut wakeup);
		for pending in wakeup.drain() {
			pending.waker().wake_by_ref();
		}
	}
}
