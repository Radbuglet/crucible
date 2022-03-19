use crate::util::number::OptionalUsize;
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone)]
pub struct FreeList<T> {
	slots: Vec<Node<T>>,
	head: OptionalUsize,
}

impl<T> Default for FreeList<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> FreeList<T> {
	pub fn new() -> Self {
		Self {
			slots: Vec::new(),
			head: OptionalUsize::NONE,
		}
	}

	pub fn add(&mut self, value: T) -> usize {
		match self.head.as_option() {
			Some(index) => {
				let target = &mut self.slots[index];
				self.head = match target {
					Node::Occupied(_) => unreachable!(),
					Node::Free(next) => *next,
				};

				*target = Node::Occupied(value);
				index
			}
			None => {
				self.slots.push(Node::Occupied(value));
				self.slots.len() - 1
			}
		}
	}

	pub fn release(&mut self, index: usize) {
		let target = self.slots.get_mut(index);

		// Ensure that we're not releasing an freed node because that would be a logical error.
		let target = match target.filter(|node| match node {
			Node::Occupied(_) => true,
			Node::Free(_) => false,
		}) {
			Some(target) => target,
			None => panic!("Cannot release a free or non-existent node!"),
		};

		// Release it!
		*target = Node::Free(self.head);
		self.head = OptionalUsize::some(index);
	}

	pub fn get(&self, index: usize) -> Option<&T> {
		self.slots.get(index).and_then(|node| match node {
			Node::Occupied(val) => Some(val),
			Node::Free(_) => None,
		})
	}

	pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
		self.slots.get_mut(index).and_then(|node| match node {
			Node::Occupied(val) => Some(val),
			Node::Free(_) => None,
		})
	}
}

impl<T> Index<usize> for FreeList<T> {
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		self.get(index).unwrap()
	}
}

impl<T> IndexMut<usize> for FreeList<T> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		self.get_mut(index).unwrap()
	}
}

#[derive(Debug, Clone)]
enum Node<T> {
	Occupied(T),
	Free(OptionalUsize),
}
