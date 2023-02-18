use std::any::type_name;

use bort::{Entity, OwnedEntity};
use crucible_util::debug::type_id::NamedTypeId;
use hashbrown::HashMap;

pub trait EntityArchetypeMarker: 'static {}

#[derive(Debug)]
pub struct EntityManager {
	archetypes: HashMap<NamedTypeId, OwnedEntity>,
}

impl EntityManager {
	pub fn archetype<M: EntityArchetypeMarker>(&mut self) -> Entity {
		self.archetypes
			.entry(NamedTypeId::of::<M>())
			.or_insert_with(|| {
				OwnedEntity::new()
					.with_debug_label(format_args!("entity archetype of {}", type_name::<M>()))
					.with_self_referential(|me| EntityArchetype {
						me,
						entities: Vec::new(),
					})
			})
			.entity()
	}

	pub fn add<M: EntityArchetypeMarker>(&mut self, entity: OwnedEntity) {
		self.archetype::<M>()
			.get_mut::<EntityArchetype>()
			.add(entity);
	}

	pub fn remove<M: EntityArchetypeMarker>(&mut self, entity: Entity) -> Option<OwnedEntity> {
		self.archetype::<M>()
			.get_mut::<EntityArchetype>()
			.remove(entity)
	}
}

#[derive(Debug)]
pub struct EntityArchetype {
	me: Entity,
	entities: Vec<OwnedEntity>,
}

impl EntityArchetype {
	pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
		self.entities.iter().map(OwnedEntity::entity)
	}

	pub fn add(&mut self, entity: OwnedEntity) {
		entity.insert(ManagedEntity {
			archetype: self.me,
			slot: self.entities.len(),
		});
		self.entities.push(entity);
	}

	pub fn remove(&mut self, entity: Entity) -> Option<OwnedEntity> {
		let Some(info) = entity.remove::<ManagedEntity>() else {
			log::warn!("Despawned unmanaged entity {entity:?}.");
			return None;
		};

		let entity = self.entities.swap_remove(info.slot);
		if let Some(moved) = self.entities.get(info.slot) {
			moved.get_mut::<ManagedEntity>().slot = info.slot;
		}

		Some(entity)
	}
}

#[derive(Debug)]
pub struct ManagedEntity {
	archetype: Entity,
	slot: usize,
}

impl ManagedEntity {
	pub fn archetype(&self) -> Entity {
		self.archetype
	}
}
