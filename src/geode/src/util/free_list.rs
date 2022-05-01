use crate::exec::accessor::{AccessorMut, ToAccessor};
use crate::util::bitset::{hibitset_length, hibitset_min_set_bit, is_valid_hibitset_index};
use hibitset::{BitSet, BitSetLike};
use std::fmt::{Debug, Formatter};
use std::iter::repeat_with;
use std::mem::MaybeUninit;
use std::ops::{Index, IndexMut};

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
	pub fn new() -> Self {
		Self::default()
	}

	pub fn reserve(&mut self, value: T) -> u32 {
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

	pub fn raw_iter(&self) -> impl Iterator<Item = u32> {
		// FIXME: Gotta love lifetimes.
		self.free.clone().iter()
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
