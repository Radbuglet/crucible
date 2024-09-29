use crucible_utils::{
    hash::{FxBuildHasher, NopHashMap},
    iter::RemoveSortedIter,
    newtypes::IndexVec,
};
use dashmap::DashMap;

use crate::{ChangeQueueFinished, Storage};

use super::{ArchetypeId, ArchetypeManager, ComponentId, Entity, EntityLocation};

// === Universe === //

#[derive(Default)]
pub struct Universe {
    storages: DashMap<ComponentId, Box<dyn StorageErased>, FxBuildHasher>,
    archetype_graph: ArchetypeManager,
    archetype_states: IndexVec<ArchetypeId, Vec<Entity>>,
    entities: NopHashMap<Entity, EntityLocation>,
}

impl Universe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, changes: &[ChangeQueueFinished]) {
        // Apply de-novo entity insertions. We do these first to avoid later modifications from
        // removing the de-novo assumption.
        for (&bundle, entities) in changes
            .iter()
            .flat_map(|v| v.added_components_de_novo.iter())
        {
            // Figure out the archetype into which all of these entities should be moved.
            let dest_arch = self
                .archetype_graph
                .find_extension(ArchetypeId::EMPTY, bundle);

            // Update the relevant storages.
            let dest_comps = self.archetype_graph.components_of(dest_arch);

            for &comp in dest_comps {
                let mut storage = self.storages.get_mut(&comp).unwrap();
                let storage = storage.value_mut();

                storage.reshape_extend_erased(dest_arch, entities);
            }

            // Update the entities' states.
            let dest_arch_state = &mut self.archetype_states[dest_arch];

            let base_dest_slot = dest_arch_state.len();
            dest_arch_state.extend_from_slice(entities);

            for (&entity, slot) in entities.iter().zip(base_dest_slot..) {
                self.entities.insert(
                    entity,
                    EntityLocation {
                        archetype: dest_arch,
                        slot,
                    },
                );
            }
        }

        // Apply entity insertions. We do these before deletions to ensure that our archetype
        // lists are correct during removals.
        for &(entity, bundle) in changes.iter().flat_map(|v| v.added_components.iter()) {
            // Figure out the entity's location.
            let entry = self.entities.entry(entity).or_insert(EntityLocation {
                archetype: ArchetypeId::EMPTY,
                slot: usize::MAX,
            });

            // Figure out the archetype into which it's being transitioned.
            let src_arch = entry.archetype;
            let dest_arch = self.archetype_graph.find_extension(src_arch, bundle);
            let src_comps = self.archetype_graph.components_of(src_arch);
            let dest_comps = self.archetype_graph.components_of(dest_arch);

            let src_loc = (entry.archetype != ArchetypeId::EMPTY).then_some(*entry);

            // Insert into storages.
            let added_comps = RemoveSortedIter::new(dest_comps.iter(), src_comps.iter());

            for &comp in added_comps {
                self.storages
                    .get_mut(&comp)
                    .unwrap()
                    .value_mut()
                    .reshape_erased(entity, src_loc, dest_arch);
            }

            // Insert into `archetype_states`.
            if let Some(src_loc) = src_loc {
                self.archetype_states[src_loc.archetype].swap_remove(src_loc.slot);
            }

            let dest_arch_ent_list = &mut self.archetype_states[dest_arch];
            let dest_slot = dest_arch_ent_list.len();
            dest_arch_ent_list.push(entity);

            // Update the archetype.
            entry.archetype = dest_arch;
            entry.slot = dest_slot;
        }

        // Apply entity deletions first since that could avoid some duplicate work while processing
        // component deletions.
        for &entity in changes.iter().flat_map(|v| v.removed_entities.iter()) {
            let Some(loc) = self.entities.remove(&entity) else {
                // The entity must have been deleted before.
                continue;
            };

            // Remove the entity from its storages.
            let comps = self.archetype_graph.components_of(loc.archetype);

            for &comp in comps {
                self.storages
                    .get_mut(&comp)
                    .unwrap()
                    .value_mut()
                    .remove_entity_erased(entity, Some(loc));
            }

            // Remove the entity from `archetype_states`.
            self.archetype_states[loc.archetype].swap_remove(loc.slot);
        }

        // Finally, apply component deletions.
        for &(entity, bundle) in changes.iter().flat_map(|v| v.removed_components.iter()) {
            let Some(loc) = self.entities.get_mut(&entity) else {
                // The entity must have been deleted before.
                continue;
            };

            // Figure out which storages have been updated.
            let src_loc = *loc;
            let dest_arch = self
                .archetype_graph
                .find_de_extension(src_loc.archetype, bundle);

            let src_comps = self.archetype_graph.components_of(src_loc.archetype);
            let dest_comps = self.archetype_graph.components_of(dest_arch);

            // Remove the components from the storages which no longer contain this value.
            let removed_comps = RemoveSortedIter::new(src_comps.iter(), dest_comps.iter());

            for &comp in removed_comps {
                self.storages
                    .get_mut(&comp)
                    .unwrap()
                    .value_mut()
                    .remove_entity_erased(entity, Some(src_loc));
            }

            // Reshape the storages that still contain the entity.
            for &comp in dest_comps {
                self.storages
                    .get_mut(&comp)
                    .unwrap()
                    .value_mut()
                    .reshape_erased(entity, Some(src_loc), dest_arch);
            }

            // Remove the entity from `archetype_states`.
            self.archetype_states[src_loc.archetype].swap_remove(src_loc.slot);

            let dest_arch_ent_list = &mut self.archetype_states[dest_arch];
            let dest_slot = dest_arch_ent_list.len();
            dest_arch_ent_list.push(entity);

            // Update the archetype.
            loc.archetype = dest_arch;
            loc.slot = dest_slot;
        }
    }
}

// === StorageErased === //

trait StorageErased {
    fn reshape_erased(&mut self, entity: Entity, src: Option<EntityLocation>, dst: ArchetypeId);

    fn reshape_extend_erased(&mut self, archetype: ArchetypeId, entities: &[Entity]);

    fn remove_entity_erased(&mut self, entity: Entity, location: Option<EntityLocation>);
}

impl<T: Storage> StorageErased for T {
    fn reshape_erased(&mut self, entity: Entity, src: Option<EntityLocation>, dst: ArchetypeId) {
        Self::reshape(self, entity, src, dst);
    }

    fn reshape_extend_erased(&mut self, archetype: ArchetypeId, entities: &[Entity]) {
        Self::reshape_extend(self, archetype, entities.iter().copied());
    }

    fn remove_entity_erased(&mut self, entity: Entity, location: Option<EntityLocation>) {
        Self::remove_entity(self, entity, location);
    }
}
