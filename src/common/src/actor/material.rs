use std::borrow::Cow;

use bort::{Entity, OwnedEntity};
use crucible_util::mem::free_list::FreeList;
use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct MaterialRegistry {
	slots: FreeList<OwnedEntity, u16>,
	id_map: HashMap<Cow<'static, str>, u16>,
}

impl MaterialRegistry {
	pub fn register(&mut self, id: impl Into<Cow<'static, str>>, descriptor: OwnedEntity) -> u16 {
		// Register in slot store
		let descriptor_ref = descriptor.entity();
		let (_, slot) = self.slots.add(descriptor);

		let id = id.into();
		let id_clone = id.clone();

		// Register in map
		if let Err(e) = self.id_map.try_insert(id, slot) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		// Attach `MaterialDescriptorBase`
		descriptor_ref.insert(MaterialDescriptorBase {
			id: Some(id_clone),
			slot,
		});

		slot
	}

	pub fn unregister(&mut self, target: Entity) {
		let MaterialDescriptorBase { id, slot } = target.remove().unwrap();
		self.id_map.remove(&id.unwrap());
		self.slots.remove(slot);
	}

	pub fn try_resolve_id(&self, id: &str) -> Option<u16> {
		self.id_map.get(id).copied()
	}

	pub fn resolve_slot(&self, slot: u16) -> Entity {
		self.slots.get(slot).entity()
	}
}

#[derive(Debug, Clone, Default)]
pub struct MaterialDescriptorBase {
	id: Option<Cow<'static, str>>,
	slot: u16,
}

impl MaterialDescriptorBase {
	pub fn id(&self) -> &str {
		self.id.as_ref().unwrap()
	}

	pub fn slot(&self) -> u16 {
		self.slot
	}
}
