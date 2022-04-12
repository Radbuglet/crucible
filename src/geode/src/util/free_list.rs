use crate::util::number::OptionalUsize;
use derive_where::derive_where;
use std::cell::UnsafeCell;
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
				self.free_head = match self.slots[index].as_occupied_raw() {
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
			.filter(|node| node.as_occupied_raw().is_ok())
			.expect("Cannot release a free or non-existent node!");

		// Release it!
		let old_target = replace(target, T::new_free(self.free_head));
		self.free_head = OptionalUsize::some(index);
		old_target.handle_occupied_unlink(self)
	}

	pub fn get_raw(&self, index: usize) -> Option<*mut T::Value> {
		self.slots
			.get(index)
			.and_then(|node| node.as_occupied_raw().ok())
	}

	pub fn get(&self, index: usize) -> Option<&T::Value> {
		self.slots
			.get(index)
			.and_then(|node| node.as_occupied_ref().ok())
	}

	pub fn get_mut(&mut self, index: usize) -> Option<&mut T::Value> {
		self.slots
			.get_mut(index)
			.and_then(|node| node.as_occupied_mut().ok())
	}
}

impl<T> IterableFreeList<T> {
	pub fn raw_iter(&self) -> RawFreeListIterator {
		RawFreeListIterator {
			head: self.occupied_head,
		}
	}

	pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
		let mut raw = self.raw_iter();

		std::iter::from_fn(move || {
			let (index, ptr) = raw.next_raw(&self)?;
			Some((index, unsafe { &*ptr }))
		})
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
		let mut raw = self.raw_iter();

		std::iter::from_fn(move || {
			let (index, ptr) = raw.next_raw(&self)?;
			Some((index, unsafe { &mut *ptr }))
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

pub unsafe trait FreeListNode: Sized {
	type OccupiedHead: Default;
	type Value;

	fn new_free(next: OptionalUsize) -> Self;
	fn new_occupied(value: Self::Value, my_index: usize, list: &mut GenericFreeList<Self>) -> Self;

	fn as_occupied_raw(&self) -> Result<*mut Self::Value, OptionalUsize>;

	fn as_occupied_ref(&self) -> Result<&Self::Value, OptionalUsize> {
		self.as_occupied_raw().map(|ptr| unsafe { &*ptr })
	}

	fn as_occupied_mut(&self) -> Result<&mut Self::Value, OptionalUsize> {
		self.as_occupied_raw().map(|ptr| unsafe { &mut *ptr })
	}

	fn handle_occupied_unlink(self, list: &mut GenericFreeList<Self>) -> Self::Value;
}

#[derive(Debug)]
pub enum SimpleFreeListNode<T> {
	Occupied(UnsafeCell<T>),
	Free(OptionalUsize),
}

impl<T: Clone> Clone for SimpleFreeListNode<T> {
	fn clone(&self) -> Self {
		use SimpleFreeListNode::*;

		match self {
			Occupied(data) => Occupied(UnsafeCell::new(unsafe { &*data.get() }.clone())),
			Free(data) => Free(*data),
		}
	}
}

unsafe impl<T> FreeListNode for SimpleFreeListNode<T> {
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
		Self::Occupied(UnsafeCell::new(value))
	}

	fn as_occupied_raw(&self) -> Result<*mut Self::Value, OptionalUsize> {
		match self {
			Self::Occupied(value) => Ok(value.get()),
			Self::Free(next) => Err(*next),
		}
	}

	fn handle_occupied_unlink(self, _list: &mut GenericFreeList<Self>) -> Self::Value {
		// Nothing to unlink, just return the value.
		match self {
			Self::Occupied(value) => value.into_inner(),
			Self::Free(_) => unreachable!(),
		}
	}
}

#[derive(Debug)]
pub struct IterableFreeListNode<T> {
	next: OptionalUsize,
	occupied: Option<(OptionalUsize, UnsafeCell<T>)>,
}

impl<T: Clone> Clone for IterableFreeListNode<T> {
	fn clone(&self) -> Self {
		Self {
			next: self.next,
			occupied: self
				.occupied
				.as_ref()
				.map(|(index, cell)| (*index, UnsafeCell::new(unsafe { &*cell.get() }.clone()))),
		}
	}
}

unsafe impl<T> FreeListNode for IterableFreeListNode<T> {
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
			occupied: Some((OptionalUsize::NONE, UnsafeCell::new(value))),
		}
	}

	fn as_occupied_raw(&self) -> Result<*mut Self::Value, OptionalUsize> {
		match &self.occupied {
			Some((_, value)) => Ok(value.get()),
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

		value.into_inner()
	}
}

#[derive(Debug, Clone)]
pub struct RawFreeListIterator {
	head: OptionalUsize,
}

impl RawFreeListIterator {
	pub fn next_raw<T>(&mut self, store: &IterableFreeList<T>) -> Option<(usize, *mut T)> {
		let curr_index = self.head.as_option()?;
		let curr_slot = &store.slots[curr_index];
		let (next, ptr) = curr_slot.occupied.as_ref().unwrap();
		self.head = *next;
		Some((curr_index, ptr.get()))
	}
}
