//! A queue of entity actions encoded as a bunch of `usize`s.
//!
//! ## Encoding scheme
//!
//! The buffer starts out in the command context.
//!
//! In the command context:
//!
//! - The most significant bit indicates whether this is a deletion (true) or an archetypal
//!   adjustment (false).
//! - The other bits indicate the entity index.
//! - The presence of this instruction transitions the decoder to archetype listing context.
//!
//! In the archetype listing context:
//!
//! - The most significant bit indicates whether this is a continuation (true) or a terminator
//!   bringing us back to the command context (false).
//! - The 2nd most significant bit indicates whether this is a deletion (true) or a deletion (false).
//!
//! This encoding scheme allows us to pack deletion and archetype data in the same buffer and
//! implement bundles in an efficient manner.

use crate::util::number::{usize_has_mask, usize_msb_mask, OptionalUsize};

#[derive(Debug, Clone, Default)]
pub struct EntityActionEncoder {
	last_slot: OptionalUsize,
	actions: Vec<usize>,
}

impl EntityActionEncoder {
	pub fn new() -> Self {
		Self::default()
	}

	fn set_target(&mut self, slot: usize) {
		if self.last_slot.as_option() != Some(slot) {
			// Push a target
			self.actions.push(slot);
			self.last_slot = OptionalUsize::some(slot);
		} else {
			// Add the continuation bit
			*self.actions.last_mut().unwrap() |= usize_msb_mask(0);
		}
	}

	pub fn add(&mut self, action: EntityAction) {
		match action {
			EntityAction::Despawn { slot } => {
				debug_assert!(slot < isize::MAX as usize);
				self.actions.push(usize_msb_mask(0) | slot);
			}
			EntityAction::AddArch { slot, arch } => {
				debug_assert!(slot < isize::MAX as usize && arch < usize_msb_mask(1));
				self.set_target(arch);
				self.actions.push(arch);
			}
			EntityAction::RemoveArch { slot, arch } => {
				debug_assert!(slot < isize::MAX as usize && arch < usize_msb_mask(1));
				self.set_target(arch);
				self.actions.push(arch | usize_msb_mask(1));
			}
		}
	}

	pub fn finish(self) -> Box<[usize]> {
		self.actions.into_boxed_slice()
	}
}

#[derive(Debug, Clone)]
pub struct EntityActionDecoder<'a> {
	mode: QueueMode,
	actions: std::slice::Iter<'a, usize>,
}

impl<'a> EntityActionDecoder<'a> {
	pub fn new(actions: &'a [usize]) -> Self {
		Self {
			mode: QueueMode::Command,
			actions: actions.into_iter(),
		}
	}

	fn parse_arch_id(slot: usize, cmd: usize) -> (bool, EntityAction) {
		let arch = cmd & !(usize_msb_mask(0) | usize_msb_mask(1));

		// Check if we have the continuation flag.
		let should_continue = usize_has_mask(cmd, usize_msb_mask(0));

		// Parse command
		let is_deletion = usize_has_mask(cmd, usize_msb_mask(1));
		let action = if is_deletion {
			EntityAction::RemoveArch { slot, arch }
		} else {
			EntityAction::AddArch { slot, arch }
		};

		(should_continue, action)
	}
}

impl Iterator for EntityActionDecoder<'_> {
	type Item = EntityAction;

	fn next(&mut self) -> Option<Self::Item> {
		match self.mode {
			QueueMode::Command => {
				let base_cmd = *self.actions.next()?;
				let base_target = base_cmd & !usize_msb_mask(0);

				if usize_has_mask(base_cmd, usize_msb_mask(0)) {
					// This is a deletion.
					Some(EntityAction::Despawn { slot: base_target })
				} else {
					// This is an archetypal adjustment.
					let arch_cmd = *self.actions.next().unwrap();

					let (should_continue, action) = Self::parse_arch_id(base_target, arch_cmd);
					if should_continue {
						self.mode = QueueMode::ArchList {
							target: base_target,
						};
					}
					Some(action)
				}
			}
			QueueMode::ArchList { target } => {
				let arch_cmd = *self.actions.next().unwrap();
				let (should_continue, action) = Self::parse_arch_id(target, arch_cmd);
				if !should_continue {
					self.mode = QueueMode::Command;
				}
				Some(action)
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum QueueMode {
	Command,
	ArchList { target: usize },
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EntityAction {
	Despawn { slot: usize },
	AddArch { slot: usize, arch: usize },
	RemoveArch { slot: usize, arch: usize },
}
