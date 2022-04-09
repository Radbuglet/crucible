use crate::util::number::OptionalUsize;
use derive_where::derive_where;
use std::mem::replace;
use std::ops::{Index, IndexMut};

pub type FreeList<T> = GenericFreeList<SimpleFreeListNode<T>>;
pub type IterableFreeList<T> = GenericFreeList<IterableFreeListNode<T>>;

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct GenericFreeList<T: FreeListNode> {
	slots: Vec<T>,
	free_head: OptionalUsize,
	occupied_head: T::OccupiedHead,
}

impl<T: FreeListNode> GenericFreeList<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(&mut self, value: T::Value) -> usize {
		match self.free_head.as_option() {
			Some(index) => {
				// Update free head
				self.free_head = match self.slots[index].as_occupied() {
					Ok(_) => unreachable!(),
					Err(next) => next,
				};

				// Generate the new slot
				let slot = T::new_occupied(value, index, self);
				self.slots[index] = slot;
				index
			}
			None => {
				let index = self.slots.len();
				let slot = T::new_occupied(value, index, self);
				self.slots.push(slot);
				index
			}
		}
	}

	pub fn release(&mut self, index: usize) -> T::Value {
		let target = self
			.slots
			.get_mut(index)
			// Ensure that we're not releasing an freed node because that would be a logical error.
			.filter(|node| node.as_occupied().is_ok())
			.expect("Cannot release a free or non-existent node!");

		// Release it!
		let old_target = replace(target, T::new_free(self.free_head));
		self.free_head = OptionalUsize::some(index);
		old_target.handle_occupied_unlink(self)
	}

	pub fn get(&self, index: usize) -> Option<&T::Value> {
		self.slots
			.get(index)
			.and_then(|node| node.as_occupied().ok())
	}

	pub fn get_mut(&mut self, index: usize) -> Option<&mut T::Value> {
		self.slots
			.get_mut(index)
			.and_then(|node| node.as_occupied_mut().ok())
	}
}

impl<T> GenericFreeList<IterableFreeListNode<T>> {
	pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
		let mut head = self.occupied_head;

		std::iter::from_fn(move || {
			let curr_index = head.as_option()?;
			let curr = &self.slots[curr_index];
			head = curr.next;

			let (_, value) = curr.occupied.as_ref().unwrap();
			Some((curr_index, value))
		})
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
		let mut head = self.occupied_head;

		std::iter::from_fn(move || {
			let curr_index = head.as_option()?;
			let curr_slot = unsafe { &mut *self.slots.as_mut_ptr().add(curr_index) };
			head = curr_slot.next;

			let (_, value) = curr_slot.occupied.as_mut().unwrap();
			Some((curr_index, value))
		})
	}
}

impl<T: FreeListNode> Index<usize> for GenericFreeList<T> {
	type Output = T::Value;

	fn index(&self, index: usize) -> &Self::Output {
		self.get(index).unwrap()
	}
}

impl<T: FreeListNode> IndexMut<usize> for GenericFreeList<T> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		self.get_mut(index).unwrap()
	}
}

pub trait FreeListNode: Sized {
	type OccupiedHead: Default;
	type Value;

	fn new_free(next: OptionalUsize) -> Self;
	fn new_occupied(value: Self::Value, my_index: usize, list: &mut GenericFreeList<Self>) -> Self;

	fn as_occupied(&self) -> Result<&Self::Value, OptionalUsize>;
	fn as_occupied_mut(&mut self) -> Result<&mut Self::Value, OptionalUsize>;

	fn handle_occupied_unlink(self, list: &mut GenericFreeList<Self>) -> Self::Value;
}

#[derive(Debug, Clone)]
pub enum SimpleFreeListNode<T> {
	Occupied(T),
	Free(OptionalUsize),
}

impl<T> FreeListNode for SimpleFreeListNode<T> {
	type OccupiedHead = ();
	type Value = T;

	fn new_free(next: OptionalUsize) -> Self {
		Self::Free(next)
	}

	fn new_occupied(
		value: Self::Value,
		_my_index: usize,
		_list: &mut GenericFreeList<Self>,
	) -> Self {
		Self::Occupied(value)
	}

	fn as_occupied(&self) -> Result<&Self::Value, OptionalUsize> {
		match self {
			Self::Occupied(value) => Ok(value),
			Self::Free(next) => Err(*next),
		}
	}

	fn as_occupied_mut(&mut self) -> Result<&mut Self::Value, OptionalUsize> {
		match self {
			Self::Occupied(value) => Ok(value),
			Self::Free(next) => Err(*next),
		}
	}

	fn handle_occupied_unlink(self, _list: &mut GenericFreeList<Self>) -> Self::Value {
		// Nothing to unlink, just return the value.
		match self {
			Self::Occupied(value) => value,
			Self::Free(_) => unreachable!(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct IterableFreeListNode<T> {
	next: OptionalUsize,
	occupied: Option<(OptionalUsize, T)>,
}

impl<T> FreeListNode for IterableFreeListNode<T> {
	type OccupiedHead = OptionalUsize;
	type Value = T;

	fn new_free(next: OptionalUsize) -> Self {
		Self {
			next,
			occupied: None,
		}
	}

	fn new_occupied(value: Self::Value, my_index: usize, list: &mut GenericFreeList<Self>) -> Self {
		// New layout:
		// <occupied_head> -> <self> -> <slots[old_occupied_head]>

		// Link head to self
		let old_occupied_head = replace(&mut list.occupied_head, OptionalUsize::some(my_index));

		// Link `old_occupied_head` to self
		if let Some(old_occupied_head) = old_occupied_head.as_option() {
			let old_occupied_head = &mut list.slots[old_occupied_head];
			let (old_occupied_head_prev, _) = old_occupied_head.occupied.as_mut().unwrap();

			*old_occupied_head_prev = OptionalUsize::some(my_index);
		}

		// "Link" self to head and `old_occupied_head`
		Self {
			next: old_occupied_head,
			occupied: Some((OptionalUsize::NONE, value)),
		}
	}

	fn as_occupied(&self) -> Result<&Self::Value, OptionalUsize> {
		match &self.occupied {
			Some((_, value)) => Ok(value),
			None => Err(self.next),
		}
	}

	fn as_occupied_mut(&mut self) -> Result<&mut Self::Value, OptionalUsize> {
		match &mut self.occupied {
			Some((_, value)) => Ok(value),
			None => Err(self.next),
		}
	}

	fn handle_occupied_unlink(self, list: &mut GenericFreeList<Self>) -> Self::Value {
		// New layout:
		// <prev> -> (removed: self) -> <next>

		let (prev, value) = self.occupied.unwrap();

		// Link prev to next
		if let Some(prev) = prev.as_option() {
			let prev = &mut list.slots[prev];
			debug_assert!(prev.occupied.is_some());
			prev.next = self.next;
		} else {
			list.occupied_head = self.next;
		}

		// Link next to prev
		if let Some(next) = self.next.as_option() {
			let next = &mut list.slots[next];
			let (next_prev, _) = next.occupied.as_mut().unwrap();
			*next_prev = prev;
		}

		value
	}
}
