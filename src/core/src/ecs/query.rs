use crate::{
	debug::lifetime::DebugLifetime,
	lang::{macros::impl_tuples, polyfill::OptionPoly},
	mem::ptr::PointeeCastExt,
};

use super::{
	entity::{ArchetypeId, Entity},
	storage::{Storage, StorageRunSlice},
};

// === Core === //

pub trait Query {
	type Iter;

	fn query_in(self, id: ArchetypeId) -> Self::Iter;
}

pub struct QueryIter<T> {
	archetype: ArchetypeId,
	slot: u32,
	max_slot: u32,
	parts: T,
}

macro impl_query($($para:ident:$field:tt),*) {
	impl<$($para: IntoQueryPartIter),*> Query for ($($para,)*) {
		type Iter = QueryIter<($($para::Iter,)*)>;

		fn query_in(self, archetype: ArchetypeId) -> Self::Iter {
			let mut slot_count = None;
			let parts = (
				$({
					let iter = self.$field.into_iter(archetype);
					let iter_slot_count = iter.max_slot(archetype);
					assert!(slot_count.p_is_none_or(|count| count == iter_slot_count));
					slot_count = Some(iter_slot_count);
					iter
				},)*
			);

			QueryIter {
				archetype,
				slot: 0,
				max_slot: slot_count.unwrap(),
				parts,
			}
		}
	}

	impl<$($para: QueryPartIter),*> Iterator for QueryIter<($($para,)*)> {
		type Item = (Entity, $($para::Output),*);

		fn next(&mut self) -> Option<Self::Item> {
			let slot = self.slot;
			if slot < self.max_slot {
				self.slot += 1;

				let res = (
					// Safety: `slot` is monotonically increasing
					$(unsafe { self.parts.$field.query_single(self.archetype, slot) },)*
				);

				let lifetime = (res.0).0;
				let entity = Entity {
					lifetime,
					arch: self.archetype,
					slot: self.slot,
				};

				Some(( entity, $((res.$field).1),* ))
			} else {
				None
			}
		}
	}
}

impl_tuples!(impl_query; no_unit);

pub trait IntoQueryPartIter {
	type Output;
	type Iter: QueryPartIter<Output = Self::Output>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter;
}

pub trait QueryPartIter {
	type Output;

	fn max_slot(&self, archetype: ArchetypeId) -> u32;

	unsafe fn query_single(
		&mut self,
		archetype: ArchetypeId,
		slot: u32,
	) -> (DebugLifetime, Self::Output);
}

// === Storage === //

impl<'a, T> IntoQueryPartIter for &'a Storage<T> {
	type Output = &'a T;
	type Iter = &'a StorageRunSlice<T>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter {
		self.get_run_slice(archetype)
	}
}

impl<'a, T> QueryPartIter for &'a StorageRunSlice<T> {
	type Output = &'a T;

	fn max_slot(&self, _archetype: ArchetypeId) -> u32 {
		self.len() as u32
	}

	unsafe fn query_single(
		&mut self,
		_archetype: ArchetypeId,
		slot: u32,
	) -> (DebugLifetime, Self::Output) {
		let slot = self
			.get(slot as usize)
			.and_then(|slot| slot.as_ref())
			.expect("missing component!");

		(slot.lifetime(), slot.value())
	}
}

impl<'a, T> IntoQueryPartIter for &'a mut Storage<T> {
	type Output = &'a mut T;
	type Iter = &'a mut StorageRunSlice<T>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter {
		self.get_run_slice_mut(archetype)
	}
}

impl<'a, T> QueryPartIter for &'a mut StorageRunSlice<T> {
	type Output = &'a mut T;

	fn max_slot(&self, _archetype: ArchetypeId) -> u32 {
		self.len() as u32
	}

	unsafe fn query_single(
		&mut self,
		_archetype: ArchetypeId,
		slot: u32,
	) -> (DebugLifetime, Self::Output) {
		self.get_mut(slot as usize)
			.and_then(|slot| slot.as_mut())
			// FIXME: Use a more standard multi-borrow pattern that doesn't run the risk of causing
			//  aliasing issues.
			.map(|slot| (slot.lifetime(), slot.value_mut().prolong_mut()))
			.expect("missing component!")
	}
}
