use std::borrow::Cow;

use bort::{Entity, OwnedEntity};
use crucible_util::mem::{free_list::FreeList, hash::FxHashMap};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MaterialId(pub u16);

impl MaterialId {
	pub const AIR: Self = Self(0);
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Material {
	pub id: MaterialId,
	pub descriptor: Entity,
}

#[derive(Debug, Default)]
pub struct MaterialRegistry {
	slots: FreeList<OwnedEntity, u16>,
	name_map: FxHashMap<Cow<'static, str>, Material>,
}

impl MaterialRegistry {
	pub fn register(
		&mut self,
		name: impl Into<Cow<'static, str>>,
		descriptor: OwnedEntity,
	) -> Material {
		// Register in slot store
		let descriptor_ref = descriptor.entity();
		let (_, slot) = self.slots.add(descriptor);

		// Construct material
		let material_id = MaterialId(slot);
		let material = Material {
			id: material_id,
			descriptor: descriptor_ref,
		};

		// Register in map
		let name = name.into();
		let name_clone = name.clone();

		if let Err(e) = self.name_map.try_insert(name, material) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		// Attach `MaterialDescriptorBase`
		descriptor_ref.insert(MaterialDescriptorBase {
			name: name_clone,
			slot: material_id,
		});

		material
	}

	pub fn unregister(&mut self, target: Entity) {
		let MaterialDescriptorBase { name, slot } = target.remove().unwrap();
		self.name_map.remove(&name);
		self.slots.remove(slot.0);
	}

	pub fn find_by_name(&self, name: &str) -> Option<Material> {
		self.name_map.get(name).copied()
	}

	pub fn find_by_id(&self, id: MaterialId) -> Material {
		let descriptor = self.slots.get(id.0).entity();
		Material { id, descriptor }
	}
}

#[derive(Debug, Clone)]
pub struct MaterialDescriptorBase {
	name: Cow<'static, str>,
	slot: MaterialId,
}

impl MaterialDescriptorBase {
	pub fn name(&self) -> &str {
		self.name.as_ref()
	}

	pub fn material(&self) -> MaterialId {
		self.slot
	}
}
