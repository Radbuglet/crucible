use std::{iter, marker::PhantomData};

use bort::{storage, CompRef, Entity, OwnedEntity};
use crucible_util::debug::type_id::NamedTypeId;
use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct ActorManager {
	archetypes: HashMap<NamedTypeId, OwnedEntity>,
	tags: HashMap<NamedTypeId, Vec<Entity>>,
}

#[derive(Debug, Default)]
struct ActorArchetype {
	entities: Vec<OwnedEntity>,
}

#[derive(Debug)]
struct Actor {
	archetype: Entity,
	slot: usize,
}

impl ActorManager {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn spawn_many<T: TagList>(
		&mut self,
		_tags: T,
		entities: impl IntoIterator<Item = OwnedEntity>,
	) {
		// Fetch the archetype for the tag set.
		//
		// N.B. for simplicity, we omitted tag de-duplication. This means that, because
		// `(Foo::TAG, Bar::TAG)` has  a different `NamedTypeId` to `(Bar::TAG, Foo::TAG)`, the two
		// archetypes will not be unified despite having identical tags. This is fine because the
		// impact of this is bounded at compile time and each spawning function will almost certainly
		// produce an entity with a different tag archetype anyways.
		let arch = self
			.archetypes
			.entry(NamedTypeId::of::<T>())
			.or_insert_with(|| {
				// Create the archetype
				let arch = OwnedEntity::new().with(ActorArchetype::default());

				// Register it into the appropriate tag lists
				for tag in T::iter_tags() {
					self.tags.entry(tag).or_default().push(arch.entity());
				}

				arch
			});

		let mut arch_state = arch.get_mut::<ActorArchetype>();

		// Register the entities
		let actors = storage::<Actor>();
		for entity in entities {
			actors.insert(
				entity.entity(),
				Actor {
					archetype: arch.entity(),
					slot: arch_state.entities.len(),
				},
			);
			arch_state.entities.push(entity);
		}
	}

	pub fn spawn<T: TagList>(&mut self, tags: T, entity: OwnedEntity) -> Entity {
		let (entity, entity_ref) = entity.split_guard();
		self.spawn_many(tags, [entity]);
		entity_ref
	}

	pub fn despawn_many(&mut self, entities: impl IntoIterator<Item = Entity>) {
		let actors = storage::<Actor>();
		let archetypes = storage::<ActorArchetype>();

		for entity in entities {
			// Get owning archetype
			let actor_info = actors.get(entity);
			let mut arch = archetypes.get_mut(actor_info.archetype);

			// Remove from archetype
			arch.entities.swap_remove(actor_info.slot);

			// Update corresponding slot of moved entity
			if let Some(moved) = arch.entities.get(actor_info.slot) {
				actors.get_mut(moved.entity()).slot = actor_info.slot;
			}
		}
	}

	pub fn despawn(&mut self, entity: Entity) {
		self.despawn_many([entity]);
	}

	pub fn tagged<T: Tag>(&self) -> impl IntoIterator<Item = Entity> + '_ {
		// Get a slice of all tagged archetype entities
		let tagged_arches = match self.tags.get(&NamedTypeId::of::<T>()) {
			Some(tag) => tag.as_slice(),
			None => &[],
		};

		// Turn that into an iterator of `CompRef<[OwnedEntity]>`.
		let arch_states = storage::<ActorArchetype>();
		let mut tagged_arches = tagged_arches
			.iter()
			.map(|tagged| arch_states.get(*tagged))
			.map(|arch| CompRef::map(arch, |arch| arch.entities.as_slice()));

		let mut curr_slice: Option<CompRef<[OwnedEntity]>> = Some(tagged_arches.next().unwrap());

		iter::from_fn(move || loop {
			// Try to get the next entity in the `curr_slice`.
			let mut next = None;
			curr_slice = Some(CompRef::map(
				curr_slice.take().unwrap(),
				|entities| match entities.split_first() {
					Some((first, rest)) => {
						next = Some(first.entity());
						rest
					}
					None => {
						next = None;
						&[]
					}
				},
			));

			if let Some(next) = next {
				return Some(next);
			}

			// Otherwise, move onto the next iterator, stopping the iterator if we've exhausted all
			// our archetypes.
			curr_slice = Some(tagged_arches.next()?);
		})
	}
}

pub trait TagList: 'static {
	type Iter: IntoIterator<Item = NamedTypeId>;

	fn iter_tags() -> Self::Iter;
}

pub trait Tag: Sized + 'static {
	const TAG: TagMarker<Self> = TagMarker { _ty: PhantomData };
}

pub struct TagMarker<T: Tag> {
	_ty: PhantomData<fn(T) -> T>,
}
