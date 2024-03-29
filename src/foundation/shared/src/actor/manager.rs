use bort::{Entity, EventTarget, OwnedEntity, VirtualTag};
use crucible_util::mem::hash::FxHashSet;

#[derive(Debug, Default)]
pub struct ActorManager {
	actors: FxHashSet<OwnedEntity>,
	tag: VirtualTag,
}

impl ActorManager {
	pub fn spawn(
		&mut self,
		event: &mut impl EventTarget<ActorSpawned>,
		actor: OwnedEntity,
	) -> Entity {
		let (actor, actor_ref) = actor.split_guard();
		actor.tag(self.tag);
		self.actors.insert(actor);
		event.fire(actor_ref, ActorSpawned);
		actor_ref
	}

	pub fn despawn(&mut self, event: &mut impl EventTarget<ActorDespawned>, actor: Entity) {
		let Some(actor) = self.actors.take(&actor) else {
			return;
		};
		actor.untag(self.tag);
		event.fire_owned(actor, ActorDespawned);
	}

	pub fn tag(&self) -> VirtualTag {
		self.tag
	}
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ActorSpawned;

#[derive(Debug)]
#[non_exhaustive]
pub struct ActorDespawned;
