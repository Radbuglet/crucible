use crate::foundation::event::EventPusher;
use crate::util::bitmask::Bitmask64;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;

#[derive(Default)]
pub struct RwLockManager {
	inner: Arc<Mutex<LockManagerInner>>,
}

impl RwLockManager {
	pub fn new() -> Self {
		Default::default()
	}
}

struct LockManagerInner {
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
	state: Arc<LockPendingState>,
}

impl Default for LockManagerInner {
	fn default() -> Self {
		Self {
			reserved_locks: Bitmask64(0),
			locks: [0; 64],
			available_locks: RwMask::EVERYTHING_AVAILABLE,
			requests: Vec::new(),
		}
	}
}

impl LockManagerInner {
	fn add_lock(&mut self) -> usize {
		// No need to reset lock state since we do that before deallocating
		self.reserved_locks
			.reserve_flag()
			.expect("Cannot allocate more than 64 locks on a pool!")
	}

	fn del_lock(&mut self, lock: usize, ev_wakeup_destroyed: &mut impl EventPusher<Event = Waker>) {
		self.reserved_locks.remove(Bitmask64::one_hot(lock));
		// TODO
		self.locks[lock] = 0;
	}

	fn try_lock_mut(&mut self, lock: usize) -> bool {
		if self.locks[lock] == 0 {
			self.locks[lock] = 1;

			let mask = Bitmask64::one_hot(lock);
			self.available_locks.read.remove(mask);
			self.available_locks.write.remove(mask);
			true
		} else {
			false
		}
	}

	fn try_lock_ref(&mut self, lock: usize) -> bool {
		if self.locks[lock] <= 0 {
			assert_ne!(self.locks[lock], isize::MIN);
			self.locks[lock] -= 1;
			self.available_locks.write.remove(Bitmask64::one_hot(lock));
			true
		} else {
			false
		}
	}

	fn unlock_mut(&mut self, lock: usize) {
		self.locks[lock] = 0;
		let mask = Bitmask64::one_hot(lock);
		self.available_locks.read.add(mask);
		self.available_locks.write.add(mask);
	}

	fn unlock_ref(&mut self, lock: usize) {
		debug_assert!(self.locks[lock] < 0);
		self.locks[lock] += 1;
		if self.locks[lock] == 0 {
			self.available_locks.write.add(Bitmask64::one_hot(lock));
		}
	}

	fn add_request(&mut self, dependencies: RwMask, waker: Waker) -> Arc<LockPendingState> {
		let state = Arc::new(LockPendingState::new(waker, self.requests.len()));
		self.requests.push(LockRequest {
			deps: dependencies,
			state: state.clone(),
		});
		state
	}

	unsafe fn del_request(&mut self, request: &LockPendingState) {
		self.remove_request(request.get_index());
		request.update_state(PendingState::Destroyed);
		request.waker.wake_by_ref();
	}

	fn poll_completed(&mut self, ev_wakeup_completed: &mut impl EventPusher<Event = LockRequest>) {
		// TODO: Mark as completed before passing on to the waker event handler.
		let available_locks = self.available_locks;
		self.remove_where(
			&|request| available_locks.contains(&request.deps),
			ev_wakeup_completed,
		)
	}

	// === Internal stuff === //

	fn remove_where<F: Fn(&LockRequest) -> bool>(
		&mut self,
		is_applicable: &F,
		on_removed: &mut impl EventPusher<Event = LockRequest>,
	) {
		let mut index = 0;
		while index < self.requests.len() {
			if is_applicable(&self.requests[index]) {
				on_removed.push(self.remove_request(index));
			// We don't increment the index here because we still have to process this element
			// in the next iteration.
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

struct LockPendingState {
	// "index" is internally synchronized with "LockManagerInner".
	index: UnsafeCell<usize>,
	// We have to use atomic operations here because the future may be polled while the manager is
	// updating the states.
	state: AtomicU8,
	waker: Waker,
}

impl LockPendingState {
	fn new(waker: Waker, index: usize) -> Self {
		Self {
			index: UnsafeCell::new(index),
			state: AtomicU8::new(PendingState::Pending as u8),
			waker,
		}
	}

	unsafe fn update_index(&self, index: usize) {
		*self.index.get() = index;
	}

	unsafe fn get_index(&self) -> usize {
		*self.index.get()
	}

	fn update_state(&self, state: PendingState) {
		// TODO: Is this ordering sufficient?
		self.state.store(state as u8, Ordering::Relaxed);
	}

	fn state(&self) -> PendingState {
		match self.state.load(Ordering::Relaxed) {
			0 => PendingState::Pending,
			1 => PendingState::Available,
			2 => PendingState::Destroyed,
			_ => panic!("Unknown PendingState discriminant!"),
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum PendingState {
	Pending = 0,
	Available = 1,
	Destroyed = 2,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct RwMask {
	read: Bitmask64,
	write: Bitmask64,
}

impl RwMask {
	pub fn contains(&self, other: &Self) -> bool {
		self.read.contains(&other.read) && self.write.contains(&other.write)
	}
}

impl RwMask {
	pub const EVERYTHING_AVAILABLE: Self = Self {
		read: Bitmask64::FULL,
		write: Bitmask64::FULL,
	};
}
