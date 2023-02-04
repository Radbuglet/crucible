use std::borrow::Cow;

use crucible_util::{mem::free_list::FreeList, object::entity::Entity};
use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct MaterialRegistry {
	slots: FreeList<Entity, u16>,
	id_map: HashMap<Cow<'static, str>, u16>,
}

impl MaterialRegistry {
	pub fn register(&mut self, id: impl Into<Cow<'static, str>>, descriptor: Entity) -> u16 {
		// Register in slot store
		let (_, slot) = self.slots.add(descriptor.into());

		let id = id.into();
		let id_clone = id.clone();

		// Register in map
		if let Err(e) = self.id_map.try_insert(id, slot) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		// Attach `BaseMaterialState`
		descriptor.insert(MaterialStateBase {
			id: Some(id_clone),
			slot,
		});

		slot
	}

	pub fn unregister(&mut self, target: Entity) {
		let MaterialStateBase { id, slot } = target.remove().unwrap();
		self.id_map.remove(&id.unwrap());
		self.slots.remove(slot);
	}

	pub fn try_resolve_id(&self, id: &str) -> Option<u16> {
		self.id_map.get(id).copied()
	}

	pub fn resolve_slot(&self, slot: u16) -> Entity {
		*self.slots.get(slot)
	}
}

#[derive(Debug, Clone, Default)]
pub struct MaterialStateBase {
	id: Option<Cow<'static, str>>,
	slot: u16,
}

impl MaterialStateBase {
	pub fn id(&self) -> &str {
		self.id.as_ref().unwrap()
	}

	pub fn slot(&self) -> u16 {
		self.slot
	}
}
