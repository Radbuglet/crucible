use std::num::NonZeroU64;

// === Internal modules === //

pub mod archetype;
pub mod entity;

// === Handle types === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	pub(super) slot: usize,
	pub(super) gen: NonZeroU64,
}

impl Entity {
	pub fn slot(&self) -> usize {
		self.slot
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ArchetypeHandle {
	pub(super) index: u32,
	pub(super) gen: NonZeroU64,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchSnapshotId(pub(super) NonZeroU64);
