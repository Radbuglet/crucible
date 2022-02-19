//! An over-engineered archetypal ECS implementation.

use crate::foundation::lock::{RwGuardMut, RwGuardRef, RwLock, RwLockManager};
use crate::util::free_list::FreeList;
use hashbrown::raw::{RawIter, RawTable};
use smallvec::SmallVec;
use std::any::Any;
use std::marker::PhantomData;
use std::mem::replace;
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::Arc;

pub const MAX_IDEAL_COMPS: usize = 16;
pub const MAX_IDEAL_ARCHETYPES: usize = 16;

// === World === //

#[derive(Debug, Clone)]
pub struct World {
	inner: Arc<RwLock<WorldInner>>,
}

#[derive(Debug)]
struct WorldInner {
	// A monotonically increasing entity generation counter.
	entity_gen: NonZeroU64,

	// A free-list of [EntitySlot]s, which allow users to map an [Entity] to its slot in its
	// containing archetype and to check that it's even still alive.
	entities: FreeList<EntitySlot>,

	// A monotonically increasing archetype ID generator.
	// See [ArchStorage.uid] for details.
	arch_id_gen: NonZeroU64,

	// Maps archetype IDs to their head.
	archetypes: FreeList<ArchetypeHead>,
}

#[derive(Debug)]
struct EntitySlot {
	// The entity's generation, used to determine whether an entity is still alive.
	gen: NonZeroU64,

	// The component's archetype. usize::MAX indicates an empty entity.
	arch_id: usize,

	// Entity index
	index: usize,
}

#[derive(Debug)]
struct ArchetypeHead {
	entities: Vec<usize>,
	components: SmallVec<[(u64, Box<dyn ComponentContainer>); MAX_IDEAL_COMPS]>,
}

trait ComponentContainer: Any {
	fn remove_comp(&mut self, index: usize);
}

impl<T> ComponentContainer for Vec<T> {
	fn remove_comp(&mut self, index: usize) {
		self.swap_remove(index)
	}
}

impl World {
	pub fn new<M: Into<RwLockManager>>(manager: M) -> Self {
		Self {
			inner: Arc::new(RwLock::new(
				manager,
				WorldInner {
					arch_id_gen: NonZeroU64::new(1).unwrap(),
					entity_gen: NonZeroU64::new(1).unwrap(),
					entities: FreeList::new(),
					archetypes: FreeList::new(),
				},
			)),
		}
	}

	pub fn use_sync(&self) -> WorldImmediate<'_> {
		WorldImmediate {
			world: self,
			guard: self.inner.lock_mut_now(),
		}
	}

	pub fn is_flushed(&self) -> bool {
		self.inner.can_lock_now_mut()
	}

	pub fn debug_assert_flushed(&self) {
		debug_assert!(
			self.is_flushed(),
			"The world has not been flushed: queued world accessors are still alive."
		);
	}
}

pub trait WorldAccess {
	fn spawn(&mut self) -> Entity;
	fn despawn(&mut self, entity: Entity);
	fn is_alive(&self, entity: Entity) -> bool;
	fn is_sync(&self) -> bool;

	fn new_storage<T>(&mut self) -> ArchStorage<T>;

	fn attach<T>(&mut self, entity: Entity, storage: &ArchStorage<T>, value: T);
	fn detach<T>(&mut self, entity: Entity, storage: &ArchStorage<T>);
	fn get_raw<T>(&self, entity: Entity, storage: &ArchStorage<T>) -> Option<NonNull<T>>;

	fn world(&self) -> &World;
}

#[derive(Debug)]
pub struct WorldImmediate<'a> {
	world: &'a World,
	guard: RwGuardMut<'a, WorldInner>,
}

impl<'a> WorldAccess for WorldImmediate<'a> {
	fn spawn(&mut self) -> Entity {
		let world = self.guard.get();
		let gen = world
			.entity_gen
			.checked_add(1)
			.expect("Failed to spawn entity: too many entities.");

		let slot = world.entities.add(EntitySlot {
			gen,
			arch_id: usize::MAX,
			index: 0,
		});

		Entity { slot, gen }
	}

	fn despawn(&mut self, entity: Entity) {
		let world = self.guard.get();

		// Fetch entity info
		let info = world
			.entities
			.get(entity.slot)
			.expect("Attempted to despawn a dead entity.");

		assert_eq!(entity.gen, info.gen, "Attempted to despawn a dead entity.");

		// Remove from archetype
		let arch = &mut world.archetypes[info.arch_id];
		arch.entities.swap_remove(info.index);
		for (_, comp) in &mut arch.components {
			comp.remove_comp(info.index);
		}

		// Update moved entity index
		if let Some(moved) = arch.entities.get(info.index) {
			world.entities[moved].index = info.index;
		}

		// Despawn entity
		world.entities.release(entity.slot);
	}

	fn is_alive(&mut self, entity: Entity) -> bool {
		self.guard
			.get()
			.entities
			.get(entity.slot)
			.filter(|info| info.gen == entity.gen)
			.is_some()
	}

	fn is_sync(&self) -> bool {
		true
	}

	fn new_storage<T>(&mut self) -> ArchStorage<T> {
		let world = self.guard.get();
		ArchStorage {
			_ty: PhantomData,
			world: self.world.clone(),
			uid: world
				.arch_id_gen
				.checked_add(1)
				.expect("Too many archetypes!"),
			containers: SmallVec::new(),
		}
	}

	fn attach<T>(&mut self, entity: Entity, storage: &ArchStorage<T>, value: T) {
		let world = self.guard.get();
	}

	fn detach<T>(&mut self, entity: Entity, storage: &ArchStorage<T>) {
		todo!()
	}

	fn get_raw<T>(&mut self, entity: Entity, storage: &ArchStorage<T>) -> Option<NonNull<T>> {
		todo!()
	}

	fn world(&self) -> &World {
		self.world
	}
}

// === Archetypal storages === //

#[derive(Debug)]
pub struct ArchStorage<T> {
	// T is covariant, is inherited during `Send + Sync` determination, and is brought into
	// consideration by the drop checker.
	_ty: PhantomData<T>,

	// The [World] this storage exists in.
	world: World,

	// A world-unique identifier for the storage.
	uid: NonZeroU64,

	// A universally-ordered list of archetypes containing this [Storage]. Dead archetypes are
	// cleaned up while walking this list.
	//
	// ## What does "universally-ordered" mean?
	//
	// Universally-ordered, in this context, means that storages will always have some universally-
	// consistent relative position in the list, as if they were sorted by their creation time.
	//
	// When archetypes are created from a list of storages, they add themselves to the end of each
	// storage's `containers` list. Because archetypes never change the list of storages they contain,
	// maintaining this invariant is practically free.
	//
	// This invariant is useful because it allows us to quickly walk along several storages'
	// `containers lists simultaneously to determine which archetypes contain the union of those
	// storages' components.
	containers: SmallVec<[ArchStorageContainer; MAX_IDEAL_ARCHETYPES]>,
}

#[derive(Debug)]
struct ArchStorageContainer {
	arch_index: usize,
	comp_index: usize,
}

// === Entities === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	slot: usize,
	gen: NonZeroU64,
}
