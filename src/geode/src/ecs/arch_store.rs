use crate::ecs::map_store::MapStorage;
use crate::ecs::world::{ArchHandle, DirtyId, StorageId, World};
use crate::util::free_list::IterableFreeList;
use std::cell::UnsafeCell;

pub struct ArchStorage<T> {
	id: StorageId,
	last_checked: DirtyId,
	locs: MapStorage<EntityLoc>,
	archetypes: IterableFreeList<StorageArchetype<T>>,
}

#[derive(Debug)]
struct StorageArchetype<T> {
	handle: ArchHandle,
	// TODO: Is there a more efficient option to keep track padding cells?
	components: Vec<UnsafeCell<Option<T>>>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct EntityLoc {
	arch_index: usize,
	comp_index: usize,
}

impl<T> ArchStorage<T> {
	pub fn flush(&mut self, world: &World) {
		//> The objective of this routine is to ensure that the layout of the world's template
		//> archetypes matches those of this storage.

		//> Ensure that every contained archetype is big enough to house template-directed components.
		// FIXME: This should create archetypes that contain this storage but aren't yet registered
		//  in the storage yet.
		for (_, store_arch) in self.archetypes.iter_mut() {
			let template_arch = match world.get_archetype(store_arch.handle) {
				Ok(arch) => arch,
				Err(_) => continue,
			};

			if store_arch.components.len() < template_arch.entities().len() {
				store_arch
					.components
					.resize_with(template_arch.entities().len(), Default::default);
			}
		}

		// For every archetype that might have slots that need to be updated with a new target
		// component...
		for (store_index, store_arch) in self.archetypes.iter() {
			// Find the template archetype for this local archetype.
			let template_arch = match world.get_archetype(store_arch.handle) {
				Ok(arch) => arch,
				// If there is no corresponding template archetype, there are no entities that need
				// to be moved into the current archetype and we can ignore it.
				Err(_) => continue,
			};

			// For every slot that could hold a potentially different entity than when we last flushed
			// the storage...
			let dirty_iter = template_arch.iter_dirties(self.last_checked);
			for (target_dirty_id, target_slot) in dirty_iter {
				// Figure out the entity the slot should hold.
				// (this cannot fail because extant slots in the template archetype always have a
				// `should_be` entry)
				let should_be_entity = template_arch.entities()[target_slot];

				//> Figure out where the `should_be_entity` currently resides.
				// The outer loop maintains the invariant that a given entity in the storage that
				// hasn't yet been processed can always be found by walking the chain of `should_be_at`
				// references with respect to an untouched `locs` map, starting with the slot
				// originally corresponding to `should_be_entity` and ending with the first slot that
				// hasn't been updated by the loop yet.
				//
				// The invariant is proven below to hold during the base case, which is when the
				// routine begins:
				//
				// Initially, no entities have been updated and the `locs` map points directly to
				// every entity. Thus, the search procedure works correctly for all of them, proving
				// the base case.
				//
				// The `n -> n+1` case is proven in the swap routine.
				//
				// This routine implements the search routine described above, which we assume
				// functions properly because of an assumed invariant.
				//
				// We're scanning for `should_be_entity`, an entity which could not yet have been
				// associated with its target cell because there can only be one cell in the template
				// requesting a specific entity.
				let mut should_be_at = *self
					.locs
					// This cannot fail because every entity contained within the template's archetype
					// must have been registered there by the `ArchStorage`, which is careful to
					// insert components in both the local storage and the template at the same time.
					.get_raw(should_be_entity);

				let should_be_data_ptr = loop {
					let should_be_store_arch = &self.archetypes[should_be_at.arch_index];
					let should_be_template_arch = world.get_archetype(should_be_store_arch.handle);

					match should_be_template_arch {
						// If we already processed the slot, it is non-terminal.
						#[rustfmt::skip]
						Ok(arch)
							if should_be_at.comp_index < arch.entities().len() && (
								should_be_at.arch_index < store_index ||
								arch.dirty_version_of(should_be_at.comp_index) > target_dirty_id
							)
						=> {
							should_be_at = *self.locs.get_raw(arch.entities()[should_be_at.comp_index]);
						}
						// Otherwise, we found a slot that hasn't yet been finalized. This must be
						// the source slot.
						_ => {
							break should_be_store_arch
								// `should_be_at` is derived from the `locs` map, which only has
								// pointers into the storage.
								.components[should_be_at.comp_index]
								.get();
						}
					};
				};

				// Find a pointer to the target's swap data.
				let target_data_ptr = store_arch
					// This is guaranteed to succeed because we already properly resized the target
					// archetype such that it is able to accommodate all of the template archetype's
					// slots.
					.components[target_slot]
					.get();

				// A bare `std::ptr::swap` call is a bit different from this pattern in that its
				// regions are allowed to partially overlap, whereas this pattern ensures that elements
				// either overlap entirely or don't overlap at all. This should hopefully help codegen
				// generate more efficient code.
				if target_data_ptr != should_be_data_ptr {
					unsafe {
						std::ptr::swap_nonoverlapping(target_data_ptr, should_be_data_ptr, 1)
					};
				}

				// Having now processed and swapped slots in the storage, we must now prove that the
				// loop invariants have been upheld.
				//
				// There are three things that have changed here:
				// a) `target_slot` has been marked as having been updated
				// b) The `target` entity (the one just processed) now contains the entity `T` and
				//    the `should_be` entity (the one into which the old target data was moved) now
				//    contains the entity `D`.
				//
				// Because `T` is now associated with its target slot, no additional considerations
				// need to be made for it. If `D == T`, then `D` would also have been processed, and
				// thus it too would not require any additional considerations.
				//
				// We now have to prove that `D` is searchable via the above algorithm, assuming
				// `D != T`.
				//
				// Because `D` was a non-processed entity when the function began, it must have been
				// searchable from its starting node, and that, in searching for `D`, we would have
				// arrived at `target`. We know from experience that, in following the finished slot
				// chain from the `target` slot to the `should_be` slot, we arrived at the non-processed
				// terminal node `should_be`. Because we only processed `target` and not `should_be`
				// during this iteration, and since we can assume `target != should_be`, `should_be`
				// is still unprocessed and is still a terminal node. Since every entity slot from
				// `locs[D]` up until before `target` is processed, searching for `D` using the
				// algorithm will bring us to `should_be`, the slot containing the proper component.
				//
				// Thus, this iteration maintained the invariant. By induction, all iterations of
				// this algorithm will properly be able to find their target entity and move it into
				// place.
				//
				// Huzzah!
			}
		}

		//> Re-associate the location map with the entries' current position in the storage.
		// Entities not associated with anything were either cleaned up by an explicit remove command,
		// or were de-spawned, a situation `MapStorage` handles automatically.
		for (store_index, store_arch) in self.archetypes.iter_mut() {
			// Find the template archetype for this local archetype.
			let template_arch = world.get_archetype(store_arch.handle).unwrap();

			// Relocate each slot
			let dirty_iter = template_arch.iter_dirties(self.last_checked);
			for (_, slot) in dirty_iter {
				self.locs.insert(
					world,
					template_arch.entities()[slot],
					EntityLoc {
						arch_index: store_index,
						comp_index: slot,
					},
				);
			}
		}

		self.last_checked = world.latest_dirty_id();

		//> Run destructors
		let mut iter = self.archetypes.raw_iter();
		while let Some((store_index, _)) = iter.next_raw(&self.archetypes) {
			let store_arch = &mut self.archetypes[store_index];

			match world.get_archetype(store_arch.handle) {
				Ok(template_arch) => {
					store_arch
						.components
						.truncate(template_arch.entities().len());
				}
				Err(_) => {
					self.archetypes.release(store_index);
				}
			};
		}
	}
}
