use crate::{debug::lifetime::DebugLifetime, lang::polyfill::OptionPoly, mem::ptr::PointeeCastExt};

use super::core::{ArchetypeId, Entity, Storage, StorageRunView};

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
					$(unsafe { self.parts.$field.get(self.archetype, slot) },)*
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

// FIXME: Use `impl_tuples` once rust-analyzer stops freaking out about it.
impl_query!(A:0);
impl_query!(A:0, B:1);
impl_query!(A:0, B:1, C:2);
impl_query!(A:0, B:1, C:2, D:3);
impl_query!(A:0, B:1, C:2, D:3, E:4);
impl_query!(A:0, B:1, C:2, D:3, E:4, F:5);
impl_query!(A:0, B:1, C:2, D:3, E:4, F:5, G: 6);
impl_query!(A:0, B:1, C:2, D:3, E:4, F:5, G: 6, H:7);
impl_query!(A:0, B:1, C:2, D:3, E:4, F:5, G: 6, H:7, I:8);

pub trait IntoQueryPartIter {
	type Output;
	type Iter: QueryPartIter<Output = Self::Output>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter;
}

pub trait QueryPartIter {
	type Output;

	fn max_slot(&self, archetype: ArchetypeId) -> u32;

	unsafe fn get(&mut self, archetype: ArchetypeId, slot: u32) -> (DebugLifetime, Self::Output);
}

// === Storage === //

impl<'a, T> IntoQueryPartIter for &'a Storage<T> {
	type Output = &'a T;
	type Iter = &'a StorageRunView<T>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter {
		self.get_run_view(archetype)
	}
}

impl<'a, T> QueryPartIter for &'a StorageRunView<T> {
	type Output = &'a T;

	fn max_slot(&self, _archetype: ArchetypeId) -> u32 {
		(*self).max_slot()
	}

	unsafe fn get(&mut self, _archetype: ArchetypeId, slot: u32) -> (DebugLifetime, Self::Output) {
		(*self).get(slot).expect("missing component!")
	}
}

impl<'a, T> IntoQueryPartIter for &'a mut Storage<T> {
	type Output = &'a mut T;
	type Iter = &'a mut StorageRunView<T>;

	fn into_iter(self, archetype: ArchetypeId) -> Self::Iter {
		self.get_run_view_mut(archetype)
	}
}

impl<'a, T> QueryPartIter for &'a mut StorageRunView<T> {
	type Output = &'a mut T;

	fn max_slot(&self, _archetype: ArchetypeId) -> u32 {
		(**self).max_slot()
	}

	unsafe fn get(&mut self, _archetype: ArchetypeId, slot: u32) -> (DebugLifetime, Self::Output) {
		(*self)
			.get_mut(slot)
			// FIXME: This is also likely illegal.
			.map(|(lt, v)| (lt, v.prolong_mut()))
			.expect("missing component!")
	}
}
