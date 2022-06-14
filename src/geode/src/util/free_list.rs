use super::bitset::{hibitset_length, hibitset_min_set_bit, is_valid_hibitset_index};
use super::number::{AtomicNZU64Generator, NonZeroU64Generator, NumberGenMut, NumberGenRef};
use crate::oop::accessor::{AccessorMut, ToAccessor};
use crossbeam::queue::SegQueue;
use derive_where::derive_where;
use hibitset::{BitSet, BitSetLike};
use std::fmt::{Debug, Formatter};
use std::iter::repeat_with;
use std::mem::MaybeUninit;
use std::num::NonZeroU64;
use std::ops::{DerefMut, Index, IndexMut};

// === Single-threaded free list === //

pub struct FreeList<T> {
	// A bitset containing the set of all free slots *actively contained within the backing vector*.
	free: BitSet,

	// A bitset containing the set of all slots reserved in the backing vector. Although functionally
	// equivalent to `BitSetNot(&self.free)`, `BitSetNot` fails to derive layers 1 through 3, making
	// this complimentary bitset more efficient to iterate than the view.
	//
	// This set is necessary, even when value iteration is not needed by the end user, to find the
	// maximum index of the storage, and to more efficiently drop the backing values once the free
	// list is dropped.
	reserved: BitSet,

	// A sparse backing vector of the free list.
	values: Vec<MaybeUninit<T>>,
}

impl<T> Default for FreeList<T> {
	fn default() -> Self {
		Self {
			free: BitSet::new(),
			reserved: BitSet::new(),
			values: Vec::new(),
		}
	}
}

impl<T> FreeList<T> {
	pub fn add(&mut self, value: T) -> u32 {
		match hibitset_min_set_bit(&self.free) {
			Some(index) => {
				self.reserved.add(index);
				self.free.remove(index);
				self.values[index as usize] = MaybeUninit::new(value);
				index
			}
			None => {
				let index = self.values.len();
				debug_assert!(is_valid_hibitset_index(index));
				let index = index as u32;

				self.reserved.add(index);
				self.values.push(MaybeUninit::new(value));
				index
			}
		}
	}

	pub fn free(&mut self, index: u32) -> Option<T> {
		// If the element actually exists.
		if self.reserved.contains(index) {
			// Unregister it and grab the value
			self.reserved.remove(index);
			let value = unsafe { self.values.get_unchecked(index as usize).assume_init_read() };

			// If the max index in the reservation bitset is less than the length of the backing
			// vector, the vector can be truncated.
			let reserved_len = hibitset_length(&self.reserved) as usize;
			if reserved_len < self.values.len() {
				self.values.truncate(reserved_len)
			} else {
				// Otherwise, we have to mark the slot as free.
				debug_assert!(reserved_len == self.values.len());
				self.free.add((reserved_len - 1) as u32);
			}

			Some(value)
		} else {
			None
		}
	}

	pub fn get(&self, index: u32) -> Option<&T> {
		if self.reserved.contains(index) {
			Some(unsafe { self.values.get_unchecked(index as usize).assume_init_ref() })
		} else {
			None
		}
	}

	pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
		if self.reserved.contains(index) {
			Some(unsafe {
				self.values
					.get_unchecked_mut(index as usize)
					.assume_init_mut()
			})
		} else {
			None
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
		#[allow(clippy::needless_borrow)] // false positive
		(&self.reserved).iter().map(move |index| {
			(index, unsafe {
				self.values.get_unchecked(index as usize).assume_init_ref()
			})
		})
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &'_ mut T)> + '_ {
		let values = self.values.as_mut_slice().to_accessor().unwrap();

		#[allow(clippy::needless_borrow)] // false positive
		(&self.reserved).iter().map(move |index| {
			(index, unsafe {
				values.get_unchecked_mut(index as usize).assume_init_mut()
			})
		})
	}
}

impl<T> Index<u32> for FreeList<T> {
	type Output = T;

	fn index(&self, index: u32) -> &Self::Output {
		match self.get(index) {
			Some(value) => value,
			None => panic!(
				"Index {index} does not point to an actively reserved element of the free list."
			),
		}
	}
}

impl<T> IndexMut<u32> for FreeList<T> {
	fn index_mut(&mut self, index: u32) -> &mut Self::Output {
		match self.get_mut(index) {
			Some(value) => value,
			None => panic!(
				"Index {index} does not point to an actively reserved element of the free list."
			),
		}
	}
}

impl<T: Debug> Debug for FreeList<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut builder = f.debug_list();

		for (index, slot) in self.values.iter().enumerate() {
			if self.reserved.contains(index as u32) {
				builder.entry(&Some(unsafe { slot.assume_init_ref() }));
			} else {
				builder.entry(&None::<&T>);
			}
		}

		builder.finish()
	}
}

impl<T: Clone> Clone for FreeList<T> {
	fn clone(&self) -> Self {
		let mut values = repeat_with(MaybeUninit::uninit)
			.take(self.values.len())
			.collect::<Vec<_>>();

		for (index, value) in self.iter() {
			values[index as usize] = MaybeUninit::new(value.clone());
		}

		Self {
			free: self.free.clone(),
			reserved: self.reserved.clone(),
			values,
		}
	}
}

impl<T> Drop for FreeList<T> {
	fn drop(&mut self) {
		#[allow(clippy::needless_borrow)] // false positive
		for index in (&self.reserved).iter() {
			unsafe {
				self.values
					.get_unchecked_mut(index as usize)
					.assume_init_drop();
			}
		}
	}
}

// === Multi-threaded free list === //

// TODO: Could we implement this with remove-only atomic hibitsets? Doing so would promote better
//  packing and generally reduce idle memory consumption.

#[derive(Debug)]
#[derive_where(Default)]
pub struct AtomicFreeList<T> {
	// A monotonically growing vector of object slots.
	slots: Vec<Option<AtomicSlot<T>>>,

	// A list of free slots at the last flush. Free slots are yielded in a LIFO manner.
	free_slots: Vec<usize>,

	// An atomic counter for the generation of the next object to be spawned.
	gen_generator: AtomicNZU64Generator,

	// The highest generation at the last list flush.
	last_flush_max_gen: u64,

	// A queue of deletion requests.
	deletions: SegQueue<Box<[usize]>>,
}

#[derive(Debug)]
struct AtomicSlot<T> {
	gen: NonZeroU64,
	meta: T,
}

impl<T> AtomicFreeList<T> {
	pub fn spawn_now<H>(&mut self, flush_handler: H) -> SlotHandle
	where
		H: FreeListFlushHandler<T>,
	{
		let entity = self.queue_spawn();
		self.flush(flush_handler);

		entity
	}

	pub fn despawn_now<H>(&mut self, handle: SlotHandle, flush_handler: H) -> T
	where
		H: FreeListFlushHandler<T>,
	{
		self.flush(flush_handler);

		let slot = &mut self.slots[handle.slot];

		// We ensure that the slot handle matches the target slot before manipulating state to make
		// this object more panic-safe.
		assert_eq!(slot.as_ref().unwrap().gen, handle.gen);

		self.free_slots.push(handle.slot);
		slot.take().unwrap().meta
	}

	pub fn queue_spawn(&self) -> SlotHandle {
		let gen = self.gen_generator.generate_ref();

		// Get the number of objects spawned since the last flush.
		let spawned = gen.get() - self.last_flush_max_gen;
		let spawned = spawned as isize;

		// Get the index of the slot from which we'll take our free object slot.
		let index_in_free_vec = self.free_slots.len() as isize - spawned;

		// Derive the slot of our new object
		let slot = if index_in_free_vec >= 0 {
			self.free_slots[index_in_free_vec as usize]
		} else {
			let end_index = self.slots.len();
			let offset = (-index_in_free_vec) as usize - 1;
			end_index + offset
		};

		SlotHandle { slot, gen }
	}

	pub fn queue_despawn_many(&self, handles: Box<[usize]>) {
		self.deletions.push(handles);
	}

	pub fn get(&self, handle: SlotHandle) -> Option<&T> {
		self.slots.get(handle.slot).and_then(|slot| {
			let slot = slot.as_ref()?;
			if slot.gen == handle.gen {
				Some(&slot.meta)
			} else {
				None
			}
		})
	}

	pub fn get_mut(&mut self, handle: SlotHandle) -> Option<&mut T> {
		self.slots.get_mut(handle.slot).and_then(|slot| {
			let slot = slot.as_mut()?;
			if slot.gen == handle.gen {
				Some(&mut slot.meta)
			} else {
				None
			}
		})
	}

	pub fn is_alive_now(&self, handle: SlotHandle) -> bool {
		self.get(handle).is_some()
	}

	pub fn is_future(&self, handle: SlotHandle) -> bool {
		handle.gen.get() > self.last_flush_max_gen
	}

	pub fn is_not_condemned(&self, handle: SlotHandle) -> bool {
		self.is_alive_now(handle) || self.is_future(handle)
	}

	pub fn is_condemned(&self, handle: SlotHandle) -> bool {
		!self.is_not_condemned(handle)
	}

	pub fn state_of(&self, handle: SlotHandle) -> SlotState {
		if self.is_alive_now(handle) {
			SlotState::Alive
		} else if self.is_future(handle) {
			SlotState::Future
		} else {
			SlotState::Condemned
		}
	}

	pub fn flush<H: FreeListFlushHandler<T>>(&mut self, mut flush_handler: H) {
		// We handle spawn requests first so despawns don't move stuff around and despawns of nursery
		// objects can be honored.
		{
			let first_gen_id = NonZeroU64::new(self.last_flush_max_gen + 1).unwrap();
			let mut id_gen = NonZeroU64Generator { next: first_gen_id };
			let max_gen_exclusive = self.gen_generator.next_value();

			// Handle slot reuses
			while id_gen.next < max_gen_exclusive && !self.free_slots.is_empty() {
				let slot = self.free_slots.pop().unwrap();
				let gen = id_gen.generate_mut();

				let meta = flush_handler.on_add(SlotHandle { slot, gen });

				self.slots[slot] = Some(AtomicSlot { gen, meta });
			}

			// Handle slot pushes
			let needed = max_gen_exclusive.get() - id_gen.next.get();
			self.slots.reserve(needed as usize);

			while id_gen.next < max_gen_exclusive {
				let slot = self.slots.len();
				let gen = id_gen.generate_mut();
				let meta = flush_handler.on_add(SlotHandle { slot, gen });

				self.slots.push(Some(AtomicSlot { gen, meta }));
			}

			self.last_flush_max_gen = self.gen_generator.next_value().get() - 1;
		}

		// Now, handle despawn requests.
		while let Some(slots) = self.deletions.pop() {
			for slot_idx in slots.iter().copied() {
				let slot = match self.slots[slot_idx].take() {
					Some(slot) => slot,
					None => continue,
				};

				flush_handler.on_remove(
					SlotHandle {
						slot: slot_idx,
						gen: slot.gen,
					},
					slot.meta,
				);
				self.free_slots.push(slot_idx);
			}
		}
	}
}

pub trait FreeListFlushHandler<T> {
	fn on_add(&mut self, handle: SlotHandle) -> T;
	fn on_remove(&mut self, handle: SlotHandle, value: T);
}

impl<T, P> FreeListFlushHandler<T> for P
where
	P: DerefMut,
	P::Target: FreeListFlushHandler<T>,
{
	fn on_add(&mut self, handle: SlotHandle) -> T {
		(&mut **self).on_add(handle)
	}

	fn on_remove(&mut self, handle: SlotHandle, value: T) {
		(&mut **self).on_remove(handle, value)
	}
}

#[derive(Debug, Copy, Clone)]
pub struct DefaultFlushHandler;

impl<T: Default> FreeListFlushHandler<T> for DefaultFlushHandler {
	fn on_add(&mut self, _handle: SlotHandle) -> T {
		Default::default()
	}

	fn on_remove(&mut self, _handle: SlotHandle, value: T) {
		drop(value);
	}
}

#[derive(Debug, Copy, Clone)]
pub struct DenyFlushHandler;

impl<T: Default> FreeListFlushHandler<T> for DenyFlushHandler {
	fn on_add(&mut self, _handle: SlotHandle) -> T {
		panic!("flushing is denied during this operation");
	}

	fn on_remove(&mut self, _handle: SlotHandle, _value: T) {
		panic!("flushing is denied during this operation");
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum SlotState {
	Alive,
	Future,
	Condemned,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct SlotHandle {
	pub slot: usize,
	pub gen: NonZeroU64,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::util::test_utils::{init_seed, rand_choice};
	use indexmap::IndexSet;
	use std::collections::HashSet;

	fn random_elem<T>(set: &IndexSet<T>) -> &T {
		set.get_index(fastrand::usize(0..set.len())).unwrap()
	}

	#[derive(Debug, Default)]
	struct Simulator {
		all: HashSet<SlotHandle>,
		alive: IndexSet<SlotHandle>,
		staged_add: IndexSet<SlotHandle>,
		staged_remove: Vec<SlotHandle>,
	}

	impl Simulator {
		pub fn queue_spawn(&mut self, handle: SlotHandle) {
			assert!(!self.all.contains(&handle));
			self.all.insert(handle);
			self.staged_add.insert(handle);
		}

		pub fn queue_despawn(&mut self, handle: SlotHandle) {
			assert!(self.all.contains(&handle));
			self.staged_remove.push(handle);
		}

		pub fn state_of(&self, handle: SlotHandle) -> SlotState {
			if self.alive.contains(&handle) {
				SlotState::Alive
			} else if self.staged_add.contains(&handle) {
				SlotState::Future
			} else {
				SlotState::Condemned
			}
		}

		pub fn flush(&mut self) {
			for add in self.staged_add.drain(..) {
				self.alive.insert(add);
			}

			for removed in self.staged_remove.drain(..) {
				self.alive.remove(&removed);
			}
		}

		pub fn random_object(&self) -> Option<SlotHandle> {
			rand_choice! {
				!self.alive.is_empty() => Some(*random_elem(&self.alive)),
				!self.staged_add.is_empty() => Some(*random_elem(&self.staged_add)),
				_ => None,
			}
		}

		pub fn assert_eq_to<M: Default>(&self, mgr: &AtomicFreeList<M>) {
			for entity in self.all.iter().copied() {
				assert_eq!(self.state_of(entity), mgr.state_of(entity));
			}
		}
	}

	#[test]
	fn auto_atomic_free_list_test() {
		init_seed();

		let mut manager = AtomicFreeList::<()>::default();
		let mut simulator = Simulator::default();

		for i in 0..1000 {
			println!("Stage {i}");

			for _ in 0..fastrand::u32(0..10) {
				let random_entity = simulator.random_object();
				rand_choice! {
					true => {
						let entity = manager.queue_spawn();
						simulator.queue_spawn(entity);
						println!("Spawning {:?}", entity);
					},
					random_entity.is_some() => {
						let target = random_entity.unwrap();
						println!("Despawning {:?}", target);
						simulator.queue_despawn(target);
						manager.queue_despawn_many(Box::new([target.slot]));
					},
					_ => unreachable!(),
				};
			}

			manager.flush(DefaultFlushHandler);
			simulator.flush();

			if i % 5 == 0 {
				simulator.assert_eq_to(&manager);
			}
		}
	}
}
