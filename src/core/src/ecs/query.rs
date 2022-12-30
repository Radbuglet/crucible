use crate::lang::macros::impl_tuples;

use super::{
	entity::{ArchetypeId, Entity},
	storage::{Storage, StorageRunSlot},
};

use std::{iter, slice};

// === Core === //

pub trait Query {
	type Iters;

	fn query_in(self, id: ArchetypeId) -> QueryIter<Self::Iters>;
}

pub trait QueryPart {
	type Iter: QueryPartIter;

	fn make_iter(part: Self, id: ArchetypeId) -> Self::Iter;
}

pub trait QueryPartIter {
	type Value;

	fn next(&mut self, id: ArchetypeId) -> Option<Option<(Entity, Self::Value)>>;
}

#[derive(Debug)]
pub struct QueryIter<T> {
	archetype: ArchetypeId,
	parts: T,
}

macro_rules! impl_query_for {
	($($para:ident:$field:tt),*) => {
		impl<$($para: QueryPart,)*> Query for ($($para,)*) {
			type Iters = ($($para::Iter,)*);

			fn query_in(self, id: ArchetypeId) -> QueryIter<Self::Iters> {
				QueryIter {
					archetype: id,
					parts: ($(QueryPart::make_iter(self.$field, id),)*),
				}
			}
		}

		impl<$($para: QueryPartIter,)*> Iterator for QueryIter<($($para,)*)> {
			type Item = (Entity, $($para::Value,)*);

			fn next(&mut self) -> Option<Self::Item> {
				#[allow(unused_mut, unused_assignments)]
				loop {
					let mut the_entity: Entity;

					let values = (
						$(match self.parts.$field.next(self.archetype)? {
							Some((entity, value)) => {
								the_entity = entity;
								value
							},
							None => continue,
						},)*
					);

					return Some((the_entity, $(values.$field),*));
				}
			}
		}
	};
}

impl_tuples!(impl_query_for; no_unit);

// === Storages === //

impl<'a, T> QueryPart for &'a Storage<T> {
	type Iter = StorageIterRef<'a, T>;

	fn make_iter(part: Self, id: ArchetypeId) -> Self::Iter {
		StorageIterRef(part.get_run_slice(id).iter().enumerate())
	}
}

pub struct StorageIterRef<'a, T>(iter::Enumerate<slice::Iter<'a, Option<StorageRunSlot<T>>>>);

impl<'a, T> QueryPartIter for StorageIterRef<'a, T> {
	type Value = &'a T;

	fn next(&mut self, arch: ArchetypeId) -> Option<Option<(Entity, Self::Value)>> {
		self.0.next().map(|(slot_idx, sparse_slot)| {
			sparse_slot.as_ref().map(|slot| {
				(
					Entity {
						lifetime: slot.lifetime(),
						arch,
						slot: slot_idx as u32,
					},
					slot.value(),
				)
			})
		})
	}
}

impl<'a, T> QueryPart for &'a mut Storage<T> {
	type Iter = StorageIterMut<'a, T>;

	fn make_iter(part: Self, id: ArchetypeId) -> Self::Iter {
		StorageIterMut(part.get_run_slice_mut(id).iter_mut().enumerate())
	}
}

pub struct StorageIterMut<'a, T>(iter::Enumerate<slice::IterMut<'a, Option<StorageRunSlot<T>>>>);

impl<'a, T> QueryPartIter for StorageIterMut<'a, T> {
	type Value = &'a mut T;

	fn next(&mut self, arch: ArchetypeId) -> Option<Option<(Entity, Self::Value)>> {
		self.0.next().map(|(slot_idx, sparse_slot)| {
			sparse_slot.as_mut().map(|slot| {
				(
					Entity {
						lifetime: slot.lifetime(),
						arch,
						slot: slot_idx as u32,
					},
					slot.value_mut(),
				)
			})
		})
	}
}
