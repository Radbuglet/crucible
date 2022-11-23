use std::{
	borrow::Borrow,
	cell::RefCell,
	cmp::Ordering,
	fmt, hash,
	num::NonZeroU64,
	sync::atomic::{AtomicU64, Ordering::SeqCst},
	thread::panicking,
};

use thiserror::Error;

use super::error::{ErrorFormatExt, ResultExt};
use crate::mem::{
	array::arr,
	pool::{GlobalPool, LocalPool},
	ptr::leak_on_heap,
};

// === Global === //

type LifetimeSlot = &'static SlotData;

#[derive(Debug)]
struct SlotData {
	gen: AtomicU64,
	deps: AtomicU64,
}

const POOL_BLOCK_SIZE: usize = 1024;

static GLOBAL_POOL: GlobalPool<LifetimeSlot> = GlobalPool::new();

thread_local! {
	static LOCAL_POOL: RefCell<LocalPool<LifetimeSlot>> = const { RefCell::new(LocalPool::new()) };
}

fn alloc_slot() -> LifetimeSlot {
	LOCAL_POOL.with(|local_pool| {
		let mut local_pool = local_pool.borrow_mut();

		local_pool.acquire(&GLOBAL_POOL, || {
			let values = leak_on_heap(arr![SlotData {
				gen: AtomicU64::new(1),
				deps: AtomicU64::new(1),
			}; POOL_BLOCK_SIZE]);

			values.into_iter().map(|v| &*v).collect()
		})
	})
}

fn free_slot(slot: LifetimeSlot) {
	LOCAL_POOL.with(|local_pool| {
		local_pool
			.borrow_mut()
			.release(&GLOBAL_POOL, POOL_BLOCK_SIZE, slot);
	});
}

// === Lifetime === //

#[derive(Debug, Clone, Error)]
#[error("attempted operation on dangling lifetime")]
pub struct DanglingLifetimeError;

#[derive(Copy, Clone)]
pub struct Lifetime {
	slot: LifetimeSlot,
	gen: NonZeroU64,
}

impl Lifetime {
	pub fn new() -> Self {
		let slot = alloc_slot();
		let gen = slot.gen.load(SeqCst);

		Self {
			slot,
			gen: NonZeroU64::new(gen).unwrap(),
		}
	}

	pub fn is_alive(self) -> bool {
		self.gen.get() == self.slot.gen.load(SeqCst)
	}

	// TODO: Verify threading semantics and potentially weaken orderings
	pub fn try_inc_dep(self) -> Result<(), DanglingLifetimeError> {
		match self.slot.deps.fetch_update(SeqCst, SeqCst, |deps| {
			if self.is_alive() {
				Some(deps + 1)
			} else {
				None
			}
		}) {
			Ok(_) => Ok(()),
			Err(_) => Err(DanglingLifetimeError),
		}
	}

	pub fn inc_dep(self) {
		self.try_inc_dep().log();
	}

	pub fn try_dec_dep(self) -> Result<(), DanglingLifetimeError> {
		match self.slot.deps.fetch_update(SeqCst, SeqCst, |deps| {
			if self.is_alive() {
				assert!(
					deps >= self.gen.get(),
					"Decremented dependency counter more times than it was incremented."
				);
				Some(deps - 1)
			} else {
				None
			}
		}) {
			Ok(_) => Ok(()),
			Err(_) => Err(DanglingLifetimeError),
		}
	}

	pub fn dec_dep(self) {
		self.try_dec_dep().log();
	}

	pub fn try_destroy(self) -> Result<(), DanglingLifetimeError> {
		let local_gen = self.gen.get();

		// First, try to invalidate all existing handles.
		let did_destroy = self
			.slot
			.gen
			.compare_exchange(local_gen, local_gen + 1, SeqCst, SeqCst)
			.is_ok();

		if !did_destroy {
			return Err(DanglingLifetimeError);
		}

		// Then, update the dependency count so it becomes a logical zero.
		// This will force all ongoing `inc/dec_dep` calls to retry their increment, making them
		// realize that they have been invalidated.
		let old_count = self.slot.deps.swap(local_gen + 1, SeqCst);

		// Release the slot to the world...
		free_slot(self.slot);

		// ...and finally, warn of a potential UAF if we just cut off some dependency. This also
		// detects concurrent `inc/dec_dep` calls, which may have completed their transaction while
		// we were destroying the lifetime.
		if old_count != local_gen {
			log::error!("Destroyed a lifetime with extant dependencies.");
		}

		Ok(())
	}

	pub fn destroy(self) {
		if let Err(err) = self.try_destroy() {
			if !panicking() {
				err.raise();
			}
		}
	}
}

impl fmt::Debug for Lifetime {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lifetime")
			.field("is_alive", &self.is_alive())
			.finish_non_exhaustive()
	}
}

// === DebugLifetime === //

#[allow(dead_code)]
mod debug_impl {
	use super::*;

	#[derive(Debug, Copy, Clone)]
	pub struct DebugLifetime(Lifetime);

	impl DebugLifetime {
		pub const IS_ENABLED: bool = true;

		pub fn new() -> Self {
			Self(Lifetime::new())
		}

		pub fn is_possibly_alive(self) -> bool {
			self.0.is_alive()
		}

		pub fn is_condemned(self) -> bool {
			!self.is_possibly_alive()
		}

		pub fn inc_dep(self) {
			self.0.inc_dep();
		}

		pub fn dec_dep(self) {
			self.0.dec_dep();
		}

		pub fn destroy(self) {
			self.0.destroy();
		}

		pub fn raw(self) -> Option<Lifetime> {
			Some(self.0)
		}
	}
}

#[allow(dead_code)]
mod release_impl {
	use super::*;

	#[derive(Debug, Copy, Clone)]
	pub struct DebugLifetime {
		_private: (),
	}

	impl DebugLifetime {
		pub const IS_ENABLED: bool = false;

		pub fn new() -> Self {
			Self { _private: () }
		}

		pub fn is_possibly_alive(self) -> bool {
			true
		}

		pub fn is_condemned(self) -> bool {
			false
		}

		pub fn inc_dep(self) {}

		pub fn dec_dep(self) {}

		pub fn destroy(self) {}

		pub fn raw(self) -> Option<Lifetime> {
			None
		}
	}
}

#[cfg(debug_assertions)]
pub use debug_impl::*;

#[cfg(not(debug_assertions))]
pub use release_impl::*;

impl Eq for DebugLifetime {}

impl PartialEq for DebugLifetime {
	fn eq(&self, _other: &Self) -> bool {
		true
	}
}

impl hash::Hash for DebugLifetime {
	fn hash<H: hash::Hasher>(&self, _state: &mut H) {}
}

impl Ord for DebugLifetime {
	fn cmp(&self, _other: &Self) -> Ordering {
		Ordering::Equal
	}
}

impl PartialOrd for DebugLifetime {
	fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
		Some(Ordering::Equal)
	}
}

// === Wrappers === //

pub trait Dependable: Copy {
	fn inc_dep(self);
	fn dec_dep(self);
}

pub trait AnyLifetime: Copy + Dependable {
	fn destroy(self);
}

impl Dependable for Lifetime {
	fn inc_dep(self) {
		// Name resolution prioritizes inherent method of the same name.
		self.inc_dep();
	}

	fn dec_dep(self) {
		// Name resolution prioritizes inherent method of the same name.
		self.dec_dep();
	}
}

impl AnyLifetime for Lifetime {
	fn destroy(self) {
		// Name resolution prioritizes inherent method of the same name.
		self.destroy()
	}
}

impl Dependable for DebugLifetime {
	fn inc_dep(self) {
		self.inc_dep();
	}

	fn dec_dep(self) {
		self.dec_dep();
	}
}

impl AnyLifetime for DebugLifetime {
	fn destroy(self) {
		// Name resolution prioritizes inherent method of the same name.
		self.destroy()
	}
}

#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct LifetimeOwner<L: AnyLifetime>(pub L);

impl<L: AnyLifetime> Drop for LifetimeOwner<L> {
	fn drop(&mut self) {
		self.0.destroy();
	}
}

#[derive(Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Dependent<L: Dependable>(L);

impl<L: Dependable> Dependent<L> {
	pub fn new(lifetime: L) -> Self {
		lifetime.inc_dep();
		Self(lifetime)
	}

	pub fn get(&self) -> L {
		self.0
	}

	pub fn into_inner(self) -> L {
		let lifetime = self.0;
		drop(self);
		lifetime
	}
}

impl<L: Dependable> Borrow<L> for Dependent<L> {
	fn borrow(&self) -> &L {
		&self.0
	}
}

impl<L: Dependable> Clone for Dependent<L> {
	fn clone(&self) -> Self {
		Self::new(self.get())
	}
}

impl<L: Dependable> Drop for Dependent<L> {
	fn drop(&mut self) {
		self.0.dec_dep();
	}
}
