use hashbrown::{HashMap, HashSet};
use std::num::NonZeroU32;

use parking_lot::Mutex;

use crate::{
	debug::{
		error::DEBUG_ASSERTIONS_ENABLED,
		label::{DebugLabel, NO_LABEL},
		lifetime::{DebugLifetime, Dependent, LifetimeLike},
	},
	mem::{drop_guard::DropOwnedGuard, free_list::PureFreeList},
};

// === Handles === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ArchetypeId {
	pub lifetime: DebugLifetime,
	pub id: NonZeroU32,
}

impl LifetimeLike for ArchetypeId {
	fn is_possibly_alive(&self) -> bool {
		self.lifetime.is_possibly_alive()
	}

	fn is_condemned(&self) -> bool {
		self.lifetime.is_condemned()
	}

	fn inc_dep(&self) {
		self.lifetime.inc_dep();
	}

	fn dec_dep(&self) {
		self.lifetime.dec_dep();
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity {
	pub lifetime: DebugLifetime,
	pub arch: ArchetypeId,
	pub slot: u32,
}

impl Entity {
	pub fn slot_usize(&self) -> usize {
		self.slot as usize
	}
}

impl LifetimeLike for Entity {
	fn is_possibly_alive(&self) -> bool {
		self.lifetime.is_possibly_alive()
	}

	fn is_condemned(&self) -> bool {
		self.lifetime.is_condemned()
	}

	fn inc_dep(&self) {
		self.lifetime.inc_dep();
	}

	fn dec_dep(&self) {
		self.lifetime.dec_dep();
	}
}

// === Containers === //

pub type EntityMap<T> = HashMap<Dependent<Entity>, T>;
pub type EntitySet = HashSet<Dependent<Entity>>;
pub type ArchetypeMap<T> = HashMap<Dependent<ArchetypeId>, T>;
pub type ArchetypeSet = HashSet<Dependent<ArchetypeId>>;

// === Archetype === //

static ARCH_ID_FREE_LIST: Mutex<PureFreeList<()>> = Mutex::new(PureFreeList::const_new());

#[derive(Debug)]
pub struct Archetype {
	id: NonZeroU32,
	lifetime: DropOwnedGuard<DebugLifetime>,
	slots: PureFreeList<DropOwnedGuard<DebugLifetime>>,
}

impl Archetype {
	pub fn new<L: DebugLabel>(name: L) -> Self {
		// Generate archetype ID
		let mut free_arch_ids = ARCH_ID_FREE_LIST.lock();
		let (_, id) = free_arch_ids.add(());
		let id = id.checked_add(1).expect("created too many archetypes.");
		let id = NonZeroU32::new(id).unwrap();

		// Construct archetype
		Self {
			id,
			lifetime: DropOwnedGuard::new(DebugLifetime::new(name)),
			slots: PureFreeList::new(),
		}
	}

	pub fn spawn<L: DebugLabel>(&mut self, name: L) -> Entity {
		let (lifetime, slot) = self
			.slots
			.add(DropOwnedGuard::new(DebugLifetime::new(name)));

		assert_ne!(slot, u32::MAX, "spawned too many entities");

		Entity {
			lifetime: **lifetime,
			arch: self.id(),
			slot,
		}
	}

	pub fn despawn(&mut self, entity: Entity) {
		if DEBUG_ASSERTIONS_ENABLED && entity.arch.id != self.id {
			log::error!(
				"Attempted to despawn {:?} from the non-owning archetype {:?}.",
				entity,
				self
			);
			return;
		}

		if entity.lifetime.is_condemned() {
			log::error!(
				"Attempted to despawn the dead entity {:?} from the archetype {:?}",
				entity,
				self
			);
			return;
		}

		let _ = self.slots.remove(entity.slot);
	}

	pub fn id(&self) -> ArchetypeId {
		ArchetypeId {
			lifetime: *self.lifetime,
			id: self.id,
		}
	}

	pub fn entities(&self) -> ArchetypeIter {
		ArchetypeIter {
			archetype: self,
			slot: 0,
		}
	}
}

impl Default for Archetype {
	fn default() -> Self {
		Self::new(NO_LABEL)
	}
}

impl Drop for Archetype {
	fn drop(&mut self) {
		let mut free_arch_ids = ARCH_ID_FREE_LIST.lock();
		free_arch_ids.remove(self.id.get() - 1);
	}
}

#[derive(Debug, Clone)]
pub struct ArchetypeIter<'a> {
	archetype: &'a Archetype,
	slot: u32,
}

impl Iterator for ArchetypeIter<'_> {
	type Item = Entity;

	fn next(&mut self) -> Option<Self::Item> {
		let slots = self.archetype.slots.slots();

		loop {
			let slot = self.slot;
			self.slot += 1;
			match slots.get(slot as usize) {
				Some(Some((lifetime, _))) => {
					break Some(Entity {
						lifetime: **lifetime,
						arch: self.archetype.id(),
						slot,
					})
				}
				Some(None) => {}
				None => break None,
			}
		}
	}
}

impl<'a> IntoIterator for &'a Archetype {
	type Item = Entity;
	type IntoIter = ArchetypeIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.entities()
	}
}
