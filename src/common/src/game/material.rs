use std::borrow::Cow;

use crucible_core::mem::free_list::FreeList;
use geode::{Dependent, Entity, Storage};
use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct MaterialRegistry {
	slots: FreeList<Dependent<Entity>, u16>,
	id_map: HashMap<Cow<'static, str>, u16>,
}

impl MaterialRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn register(
		&mut self,
		(base_states,): (&mut Storage<BaseMaterialState>,),
		id: impl Into<Cow<'static, str>>,
		descriptor: Entity,
	) -> u16 {
		// Register in slot store
		let (_, slot) = self.slots.add(descriptor.into());

		let id = id.into();
		let id_clone = id.clone();

		// Register in map
		if let Err(e) = self.id_map.try_insert(id, slot) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		// Attach `BaseMaterialState`
		base_states.add(descriptor, BaseMaterialState { id: id_clone, slot });

		slot
	}

	pub fn unregister(
		&mut self,
		(base_states,): (&mut Storage<BaseMaterialState>,),
		target: Entity,
	) {
		let BaseMaterialState { id, slot } = base_states.try_remove(target).unwrap();
		self.id_map.remove(&id);
		self.slots.remove(slot);
	}

	pub fn try_resolve_id(&self, id: &str) -> Option<u16> {
		self.id_map.get(id).copied()
	}

	pub fn resolve_slot(&self, slot: u16) -> Entity {
		self.slots.get(slot).get()
	}
}

#[derive(Debug, Clone)]
pub struct BaseMaterialState {
	id: Cow<'static, str>,
	slot: u16,
}

impl BaseMaterialState {
	pub fn id(&self) -> &str {
		&self.id
	}

	pub fn slot(&self) -> u16 {
		self.slot
	}
}
