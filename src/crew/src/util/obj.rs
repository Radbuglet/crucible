use bumpalo::Bump;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::needs_drop;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[derive(Default)]
pub struct Stage {
	entries: RefCell<HashMap<(Entity, TypeId), StageEntry>>,
	bump: Bump,
}

struct StageEntry {
	ptr: *mut (),
	drop_fn: Option<unsafe fn(*mut ())>,
}

impl Drop for Stage {
	fn drop(&mut self) {
		for entry in self.entries.get_mut().values_mut() {
			if let Some(drop_fn) = entry.drop_fn {
				unsafe { (drop_fn)(entry.ptr) }
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	id: NonZeroU64,
}

impl Default for Entity {
	fn default() -> Self {
		static ID_GEN: AtomicU64 = AtomicU64::new(1);

		Self {
			id: NonZeroU64::new(ID_GEN.fetch_add(1, AtomicOrdering::Relaxed))
				.expect("spawned too many entities"),
		}
	}
}

impl Entity {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn add<T: 'static>(self, stage: &Stage, value: T) -> &T {
		let mut entries = stage.entries.borrow_mut();
		let key = (self, TypeId::of::<T>());
		assert!(!entries.contains_key(&key));

		let comp_ref = stage.bump.alloc(value);
		let comp_ptr = comp_ref as *mut T as *mut ();

		// Safety: Reborrowing raw pointers as references does not strip them of their mutable
		// powers. Logically, when we drop the `Stage`, we're borrowing the raw pointer as mutable,
		// killing off any remaining immutable references. However, all of these references are
		// limited to the lifetime of the stage anyways, so users can never use these dead references.
		//
		// Thanks, stacked borrows!
		let comp_ref = &*comp_ref;

		unsafe fn drop<T>(ptr: *mut ()) {
			(ptr as *mut T).drop_in_place()
		}

		entries.insert(
			key,
			StageEntry {
				ptr: comp_ptr,
				drop_fn: if needs_drop::<T>() {
					Some(drop::<T>)
				} else {
					None
				},
			},
		);
		comp_ref
	}

	pub fn try_get<T: 'static>(self, stage: &Stage) -> Option<&T> {
		stage
			.entries
			.borrow()
			.get(&(self, TypeId::of::<T>()))
			.map(|entry| {
				let ptr = entry.ptr as *const T;
				unsafe { &*ptr }
			})
	}

	pub fn get<T: 'static>(self, stage: &Stage) -> &T {
		self.try_get(stage).unwrap()
	}
}
