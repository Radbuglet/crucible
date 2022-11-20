use super::core::{ArchetypeId, Entity};

pub trait Query {
	type Iter;

	fn query_in(self, archetype: ArchetypeId) -> Self::Iter;
}

pub trait IntoQueryPart {
	type Iter: QueryPart<Output = Self::Output>;
	type Output;

	fn into_query_part(self) -> Self::Iter;
}

pub trait QueryPart {
	type Output;

	fn is_main(&self) -> bool;
	fn next_main(&mut self, archetype: ArchetypeId) -> Option<(Entity, Self::Output)>;

	fn next_puppet(&mut self, target: Entity, archetype: ArchetypeId) -> Option<Self::Output>;
	fn process_missing(&mut self, target: Entity, archetype: ArchetypeId);
}

// === EntityQuery and ArchQuery === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct EntityQuery;

impl IntoQueryPart for EntityQuery {
	type Iter = Self;
	type Output = Entity;

	fn into_query_part(self) -> Self::Iter {
		self
	}
}

impl QueryPart for EntityQuery {
	type Output = Entity;

	fn is_main(&self) -> bool {
		false
	}

	fn next_main(&mut self, _archetype: ArchetypeId) -> Option<(Entity, Self::Output)> {
		unimplemented!()
	}

	fn next_puppet(&mut self, target: Entity, _archetype: ArchetypeId) -> Option<Self::Output> {
		Some(target)
	}

	fn process_missing(&mut self, _target: Entity, _archetype: ArchetypeId) {
		unimplemented!()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct ArchQuery;

impl IntoQueryPart for ArchQuery {
	type Iter = Self;
	type Output = ArchetypeId;

	fn into_query_part(self) -> Self::Iter {
		self
	}
}

impl QueryPart for ArchQuery {
	type Output = ArchetypeId;

	fn is_main(&self) -> bool {
		false
	}

	fn next_main(&mut self, _archetype: ArchetypeId) -> Option<(Entity, Self::Output)> {
		unimplemented!()
	}

	fn next_puppet(&mut self, _target: Entity, archetype: ArchetypeId) -> Option<Self::Output> {
		Some(archetype)
	}

	fn process_missing(&mut self, _target: Entity, _archetype: ArchetypeId) {
		unimplemented!()
	}
}

// === OptionalQuery === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct OptionalQuery<T>(pub T);

impl<T: QueryPart> QueryPart for OptionalQuery<T> {
	type Output = T::Output;

	fn is_main(&self) -> bool {
		false
	}

	fn next_main(&mut self, _archetype: ArchetypeId) -> Option<(Entity, Self::Output)> {
		unimplemented!()
	}

	fn next_puppet(&mut self, target: Entity, archetype: ArchetypeId) -> Option<Self::Output> {
		self.0.next_puppet(target, archetype)
	}

	fn process_missing(&mut self, _target: Entity, _archetype: ArchetypeId) {
		// ignored
	}
}

// === Storage Queries === //

// TODO
