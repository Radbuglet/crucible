//! A queue of entity actions encoded as a bunch of `u64`s.
//!
//! ## Encoding scheme
//!
//! The buffer starts out in the command context.
//!
//! In the command context:
//!
//! - Indicates the target entity slot and transitions to the storage listing context.
//!
//! In the storage listing context:
//!
//! - The most significant bit indicates whether this is a continuation (true) or a terminator
//!   bringing us back to the command context (false).
//! - The 2nd most significant bit indicates whether this is a deletion (true) or a deletion (false).
//!
//! This encoding scheme allows us to pack deletion and storage data in the same buffer and
//! implement bundles in an efficient manner.

use crate::util::number::{u64_has_mask, u64_msb_mask, OptionalUsize};

#[derive(Debug, Clone, Default)]
pub struct EntityActionEncoder {
	last_slot: OptionalUsize,
	actions: Vec<u64>,
}

impl EntityActionEncoder {
	fn set_target(&mut self, slot: usize) {
		if self.last_slot.as_option() != Some(slot) {
			// Push a target
			self.actions.push(slot as u64);
			self.last_slot = OptionalUsize::some(slot);
		} else {
			// Add the continuation bit
			*self.actions.last_mut().unwrap() |= u64_msb_mask(0);
		}
	}

	pub fn add(&mut self, action: ReshapeAction) {
		match action {
			ReshapeAction::Add { slot, storage } => {
				debug_assert!(slot < isize::MAX as usize && storage < u64_msb_mask(1));
				self.set_target(slot);
				self.actions.push(storage as u64);
			}
			ReshapeAction::Remove { slot, storage } => {
				debug_assert!(slot < isize::MAX as usize && storage < u64_msb_mask(1));
				self.set_target(slot);
				self.actions.push(storage | u64_msb_mask(1));
			}
		}
	}

	pub fn finish(self) -> Box<[u64]> {
		self.actions.into_boxed_slice()
	}
}

#[derive(Debug, Clone)]
pub struct EntityActionDecoder<'a> {
	build_to: OptionalUsize,
	actions: std::slice::Iter<'a, u64>,
}

impl<'a> EntityActionDecoder<'a> {
	pub fn new(actions: &'a [u64]) -> Self {
		Self {
			build_to: OptionalUsize::NONE,
			actions: actions.into_iter(),
		}
	}

	fn parse_storage_id(slot: usize, cmd: u64) -> (bool, ReshapeAction) {
		let storage = cmd & !(u64_msb_mask(0) | u64_msb_mask(1));

		// Check if we have the continuation flag.
		let should_continue = u64_has_mask(cmd, u64_msb_mask(0));

		// Parse command
		let is_deletion = u64_has_mask(cmd, u64_msb_mask(1));
		let action = if is_deletion {
			ReshapeAction::Remove { slot, storage }
		} else {
			ReshapeAction::Add { slot, storage }
		};

		(should_continue, action)
	}
}

impl Iterator for EntityActionDecoder<'_> {
	type Item = ReshapeAction;

	fn next(&mut self) -> Option<Self::Item> {
		match self.build_to.as_option() {
			None => {
				let target = *self.actions.next()?;
				debug_assert!(target <= usize::MAX as u64);
				let target = target as usize;

				let storage_cmd = *self.actions.next().unwrap();

				let (should_continue, action) = Self::parse_storage_id(target, storage_cmd);
				if should_continue {
					self.build_to = OptionalUsize::some(target);
				}
				Some(action)
			}
			Some(target) => {
				let arch_cmd = *self.actions.next().unwrap();
				let (should_continue, action) = Self::parse_storage_id(target, arch_cmd);
				if !should_continue {
					self.build_to = OptionalUsize::NONE;
				}
				Some(action)
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum ReshapeAction {
	Add { slot: usize, storage: u64 },
	Remove { slot: usize, storage: u64 },
}
