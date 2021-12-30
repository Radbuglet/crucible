//! An over-engineered bespoke archetypal ECS implementation.
//!
//! Non-trivial operations:
//!
//! - Detect if any entity is alive
//! - (Re)construct an entity from a list of storages and their components
//! - Fetch a component from a storage given the entity
//! - Query a list of storages
//! - Destroy a storage, reshaping member entities
//!

use crate::util::free_list::FreeList;
use smallvec::SmallVec;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ptr::NonNull;
use std::sync::Arc;

/// The maximum number of components which can be attached to an entity without triggering the slow
/// path.
pub const MAX_IDEAL_COMPS: usize = 16;

/// The maximum number of archetypes which can be attached to a storage without triggering the slow
/// path.
pub const MAX_IDEAL_ARCHETYPES: usize = 16;

// === World === //

#[derive(Debug)]
pub struct World {
	inner: Arc<WorldInner>,
}

#[derive(Debug)]
struct WorldInner {
	// A free-list of [EntitySlot]s, which allow users to map an [Entity] to its slot in its
	// containing archetype and to check that it's even still alive.
	entities: FreeList<EntitySlot>,
}

#[derive(Debug)]
struct EntitySlot {
	// The entity's generation, used to determine whether an entity is still alive.
	gen: NonZeroU64,

	archetype_id: u64,

	// The archetype block containing this entity.
	block: NonNull<BlockHeader>,

	// This entity's index within the block.
	block_index: usize,
}

#[derive(Debug)]
pub struct BlockHeader {}

// === Storages === //

#[derive(Debug)]
pub struct Storage<T> {
	// T is covariant, is inherited during `Send + Sync` determination, and is brought into
	// consideration by the drop checker.
	_ty: PhantomData<T>,

	// The [World] this storage exists in.
	world: World,

	// The unique ID of the `storage` within the world. This is necessary to find the rank of an
	// entity's corresponding component within an archetype.
	id: NonZeroU64,

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
	containers: SmallVec<[ContainedBy; MAX_IDEAL_ARCHETYPES]>,
}

#[derive(Debug)]
struct ContainedBy {
	// Used to perform a binary search for a given archetype.
	archetype_id: u64,

	// The index of the archetype in the `World.archetypes` free-list.
	head_block: usize,

	// The index of the rank in the `BlockHeader.rank_offsets` list.
	rank_index: usize,
}

// === Entities === //

#[derive(Debug)]
pub struct Entity {
	slot: usize,
	gen: NonZeroU64,
}
