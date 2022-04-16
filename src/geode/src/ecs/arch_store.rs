use crate::ecs::map_store::MapStorage;
use crate::ecs::world::{
	ArchHandle, DirtyId, StorageId, World, WorldArchDirtyIter, WorldArchetype,
};
use crate::util::free_list::IterableFreeList;
use crate::util::iter_ext::VecFilterExt;
use crate::util::usually_init::UsuallyInit;

pub struct ArchStorage<T> {
	id: StorageId,
	locs: MapStorage<EntityLoc>,
	archetypes: IterableFreeList<StorageArchetype<T>>,
}

#[derive(Debug)]
struct StorageArchetype<T> {
	handle: ArchHandle,
	last_checked: DirtyId,
	components: Vec<UsuallyInit<T>>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct EntityLoc {
	arch_index: usize,
	comp_index: usize,
}

impl<T> ArchStorage<T> {
	// Welcome to hell!
	//
	// At a first glance, this method would seem to be implementing a permutation application
	// algorithm with support for dropping elements, but this is not the case.
	//
	// The key simplification comes from the fact that we can always find the most recently updated
	// slot in the vector. This means that we can always figure out the head of a not-always-cyclic
	// permutation chain, meaning that unterminated permutation chains will necessarily drop their
	// head element since nothing could possibly point to it.
	pub fn flush(&mut self, world: &World) {
		use std::ptr;

		//> Ensure that every archetype is sufficiently sized to accommodate component data past their
		//> ends. This routine does not shrink component storages if they're too big because we still
		//> need that data. Shrinking is rather handled during the slot purging routine.
		for (_, arch) in self.archetypes.iter_mut() {
			let template_len = world.get_archetype(arch.handle).map_or(0, |template| template.entities().len());
			if arch.components.len() < template_len {
				arch.components.resize_with(template_len, || unsafe { UsuallyInit::uninit() });
			}
		}

		//> Collect dirty slot iterators for each archetype.
		struct ArchIterState<'a, T> {
			store_arch_index: usize,
			store_arch: &'a StorageArchetype<T>,
			world_arch: &'a WorldArchetype,
			iter: WorldArchDirtyIter<'a>,
		}

		// These iterators provide a list of slots that might have a different target entity than
		// is currently present in them. Slots without a target entity are not included.
		// FIXME: This actually needs to iterate over *all* possible archetypes that could contain
		//  the storage, rather than the ones that actually exist in the storage.
		let mut target_iters = self
			.archetypes
			.iter()
			.filter_map(|(store_arch_index, store_arch)| {
				let world_arch = world
					.get_archetype(store_arch.handle)
					// Ignoring archetypes without a template is fine as this follows the semantics
					// described above.
					.ok()?;

				let iter = world_arch.iter_dirties(store_arch.last_checked);

				// We do this check here, even though the first iteration of the dirty slot handler
				// could handle it, so we can avoid a backing heap allocation for no-op flushes.
				if iter.clone().next().is_none() {
					return None;
				}

				Some(ArchIterState {
					store_arch_index,
					store_arch,
					world_arch,
					iter,
				})
			})
			.collect::<Vec<_>>();

		// While we still have dirty slots...
		loop {
			//> Find the greatest dirty ID
			let arch_iter_index = {
				let mut greatest_entry: Option<(DirtyId, usize)> = None;

				target_iters.retain_enumerated(|entry_index, entry| {
					// Fetch the next ID in this iterator
					let head_id = match entry.iter.clone().next() {
						Some((id, _)) => id,
						None => return false,
					};

					// If it's greater than the greatest entry we found, mark it.
					if greatest_entry.map_or(true, |(greatest, _)| head_id.get() > greatest.get()) {
						greatest_entry = Some((head_id, entry_index));
					}

					true
				});

				match greatest_entry {
					Some((_, index)) => index,
					None => break,
				}
			};

			//> Handle permutation chain moves

			// The location at the beginning of the chain.
			let start_loc: EntityLoc;

			// The data of the entity at the beginning of the chain
			let start_data: T;

			// The pointer to the slot into which we're moving data. This pointee should be considered
			// logically uninitialized.
			let mut target_ptr: *mut T;

			// The location of the entity whose data that should be in the target.
			let mut should_be_loc: EntityLoc;

			// Determine initial state
			{
				// Determine starting location
				let entry = &mut target_iters[arch_iter_index];

				let (_, dirty_slot_index) = entry
					.iter
					.next()
					// We already discard all `arch_dirty_iter` without a succeeding element so this
					// is guaranteed to be valid.
					.unwrap();

				start_loc = EntityLoc {
					arch_index: entry.store_arch_index,
					comp_index: dirty_slot_index,
				};

				// Determine what the target should be.
				let should_be_entity = entry
					.world_arch
					// This will never fail because only slots with an actual `should_be` entity can
					// be marked as dirty.
					.entities()[dirty_slot_index];

				// Update the location of the entity that should be here.
				// Yes, this doesn't unlink the `target` entity currently being evicted. See the
				// "three cases" comment later on in the file for an explanation on why this is fine.
				self.locs.insert(world, should_be_entity, start_loc);

				// This is valid because entities can only be added to this storage's archetype
				// through its dedicated `insert` method, which tracks the component in a scratch
				// space.
				should_be_loc = *self.locs.get_raw(should_be_entity);

				// If the `start_loc` is equal to `should_be_loc`, nothing more needs to be moved in
				// this chain.
				if start_loc == should_be_loc {
					continue;
				}

				// Otherwise, determine the data pointer.
				target_ptr =
					// This promotion is valid, despite not having a mutable reference to the
					// `store_arch`, because the returned raw pointer to the heap wasn't derived from
					// a immutable reference... I think.
					unsafe { entry.store_arch.components.as_ptr().add(dirty_slot_index) } as *mut T;

				// And backup the start data in preparation for the copies.
				// (good news, `target_ptr`'s pointee is now logically uninitialized! If something
				// breaks while still in this state, we'll have some *very funky* behavior)
				start_data = unsafe { ptr::read(target_ptr) };
			}

			// Until we reach a terminator...
			loop {
				// Move `should_be` data into `target`.
				let should_be_local_arch = self.archetypes.get(should_be_loc.arch_index).unwrap();
				let should_be_ptr = unsafe {
					should_be_local_arch
						.components
						.as_ptr()
						.add(should_be_loc.comp_index)
				};
				let should_be_ptr = should_be_ptr as *mut T;

				unsafe {
					ptr::copy(should_be_ptr, target_ptr, 1);
				}

				// Make `should_be` the new `target`
				target_ptr = should_be_ptr;

				// Determine the entity that should be here.
				let should_be_entity_or_none = world
					// Try to get the archetype.
					.get_archetype(should_be_local_arch.handle)
					.ok()
					// Try to get the entity that should be in that slot.
					.and_then(|world_arch| {
						world_arch.entities().get(should_be_loc.comp_index).copied()
					});

				// We have three cases...
				match should_be_entity_or_none {
					Some(should_be_entity) => {
						let loc = *self.locs.get_raw(should_be_entity);

						// Move the target entity's location.
						self.locs.insert(world, should_be_entity, loc);

						if loc == start_loc {
							// ...we went back to the beginning slot, meaning that we're now moving
							// `start_data` into `target_ptr`, thus completing the chain.

							unsafe { ptr::write(target_ptr, start_data) };
							break;
						} else {
							// ...we still have a non-terminal node that needs to be filled out with
							// something.
							// Note: we know `loc` is not going to be any of the locs we have previously
							// visited (including this loc) because a given entity can only show up
							// in one specific location. This means that a) `start_loc` is the only
							// location we have to check for repeats and b) this algorithm will
							// terminate.

							// And move on to this next location.
							should_be_loc = loc;
						}
					}

					// ...nothing should be in the target slot. Ignore it, put `start_data` in the
					// dead slot and let the purging routine drop it.
					//
					// "Why do we write the element back instead of just dropping it immediately?"
					//
					// The purging routine expects that all the arrays are property initialized. This
					// is required because the purging routine actually does have to drop some amount
					// of elements that were not part of a move chain. Furthermore, doing so would
					// mean that the storage would be being left in a super dangerous state where
					// some of the vector elements are logically uninitialized and dangerous to give
					// to the user. Dropping components immediately would mean that we would have to
					// run user code, which could panic and allow this invalid state to become
					// accessible to the user. We're better off making this entire section panic-unsafe
					// and being super careful to ensure that the users never take over.
					//
					// "What about updating the location `HashMap` for entities who do not have a
					// `should_be` location?"
					//
					// There are two ways in which an entity previously in this `Storage` can stop
					// having a `should_be` location: a) the component was removed or b) the entity
					// was deleted.
					//
					// Component removal will always take a mutable reference to this storage, which
					// will always allow us to find the entity from which the component was removed
					// and remove it from the location map immediately.
					//
					// Entity removal is reflected by queries in a `MapStorage`, meaning that the
					// entries don't need to be updated.
					//
					// "But won't loosing track of this component be bad if it turns out this value
					// is needed somewhere else?"
					//
					// Luckily for the sanity of the author, the `World` conveniently sorts archetype
					// modifications by when they were performed. That means that the last modification
					// moving an entity to a new archetype is the one that the algorithm will detect
					// first. *i.e.* We will always process the head of each of these movement lists
					// so nothing could possibly point to `start_data`.
					None => {
						unsafe { ptr::write(target_ptr, start_data) };
						break;
					}
				}
			}
		}

		//> Purge dead archetype slots
		let mut iter = self.archetypes.raw_iter();

		while let Some((index, _)) = iter.next_raw(&self.archetypes) {
			let local = &mut self.archetypes[index];
			match world.get_archetype(local.handle) {
				Ok(template) => local.components.truncate(template.entities().len()),
				Err(_) => {
					self.archetypes.release(index);
				},
			}
		}
	}
}
