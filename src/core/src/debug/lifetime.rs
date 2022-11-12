use core::fmt;
use std::{
	cmp::Ordering,
	hash,
	num::NonZeroU64,
	sync::atomic::{AtomicU64, Ordering::Relaxed},
};

// === Global === //

type LifetimeSlot = &'static AtomicU64;

mod slot_db {
	use std::sync::Mutex;

	use crate::{debug::error::ResultExt, mem::array::arr};

	use super::*;

	static FREE_SLOTS: Mutex<Vec<LifetimeSlot>> = Mutex::new(Vec::new());

	pub fn alloc() -> LifetimeSlot {
		// TODO: Implement local cache to avoid excessive lock contention
		let mut free_slots = FREE_SLOTS.lock().unwrap_pretty();

		if let Some(slot) = free_slots.pop() {
			slot
		} else {
			let block = Box::leak(Box::new(arr![AtomicU64::new(1); 1024]));
			free_slots.extend(block.into_iter().map(|r| &*r));
			free_slots.pop().unwrap()
		}
	}

	pub fn free(slot: LifetimeSlot) {
		let mut free_slots = FREE_SLOTS.lock().unwrap_pretty();
		free_slots.push(slot);
	}
}

// === Lifetime === //

#[derive(Copy, Clone)]
pub struct Lifetime {
	slot: LifetimeSlot,
	gen: NonZeroU64,
}

impl Lifetime {
	pub fn new() -> Self {
		let slot = slot_db::alloc();
		let gen = slot.load(Relaxed);

		Self {
			slot,
			gen: NonZeroU64::new(gen).unwrap(),
		}
	}

	pub fn is_alive(self) -> bool {
		self.gen.get() == self.slot.load(Relaxed)
	}

	pub fn destroy(self) -> bool {
		let did_destroy = self
			.slot
			.compare_exchange(self.gen.get(), self.gen.get() + 1, Relaxed, Relaxed)
			.is_ok();

		if did_destroy {
			slot_db::free(self.slot);
			true
		} else {
			false
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

impl Default for Lifetime {
	fn default() -> Self {
		Self::new()
	}
}

// === DebugLifetime === //

#[allow(dead_code)]
mod debug_impl {
	use super::*;

	#[derive(Debug, Copy, Clone, Default)]
	pub struct DebugLifetime(Lifetime);

	impl DebugLifetime {
		pub const IS_ENABLED: bool = true;

		pub fn new() -> Self {
			Self::default()
		}

		pub fn is_possibly_alive(self) -> bool {
			self.0.is_alive()
		}

		pub fn is_condemned(self) -> bool {
			!self.is_possibly_alive()
		}

		pub fn raw(self) -> Option<Lifetime> {
			Some(self.0)
		}

		pub fn destroy(self) {
			self.0.destroy();
		}
	}
}

#[allow(dead_code)]
mod release_impl {
	use super::*;

	#[derive(Debug, Copy, Clone, Default)]
	pub struct DebugLifetime {
		_private: (),
	}

	impl DebugLifetime {
		pub const IS_ENABLED: bool = false;

		pub fn new() -> Self {
			Self::default()
		}

		pub fn is_possibly_alive(self) -> bool {
			true
		}

		pub fn is_condemned(self) -> bool {
			false
		}

		pub fn raw(self) -> Option<Lifetime> {
			None
		}

		pub fn destroy(self) {}
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
