use std::{fmt, hash};

use bytemuck::TransparentWrapper;

#[derive(Debug)]
pub struct FreeList<T, H: Handle = FreeListHandle> {
	slots: Vec<Option<(T, H::Meta)>>,
	free: Vec<usize>, // TODO: Improve slot packing with a hibitset.
	state: H::State,
}

pub trait NewtypedHandle:
	Sized + fmt::Debug + Copy + hash::Hash + Eq + TransparentWrapper<Self::DeferTo>
{
	type DeferTo: Handle;
}

pub trait Handle: Sized + fmt::Debug + Copy + hash::Hash + Eq {
	type State: fmt::Debug;
	type Meta;

	fn default_state() -> Self::State;

	fn slot_occupied(state: &mut Self::State, slot: usize) -> (Self, Self::Meta);

	fn slot_freed(state: &mut Self::State, handle: Self, meta: Self::Meta);

	fn validate_handle(state: &Self::State, handle: Self, meta: &Self::Meta) -> bool;

	fn slot(&self) -> usize;
}

impl<H: NewtypedHandle> Handle for H {
	type State = <H::DeferTo as Handle>::State;
	type Meta = <H::DeferTo as Handle>::Meta;

	fn default_state() -> Self::State {
		<H::DeferTo>::default_state()
	}

	fn slot_occupied(state: &mut Self::State, slot: usize) -> (Self, Self::Meta) {
		let (handle, meta) = <H::DeferTo>::slot_occupied(state, slot);
		(H::wrap(handle), meta)
	}

	fn slot_freed(state: &mut Self::State, handle: Self, meta: Self::Meta) {
		<H::DeferTo>::slot_freed(state, H::peel(handle), meta);
	}

	fn validate_handle(state: &Self::State, handle: Self, meta: &Self::Meta) -> bool {
		<H::DeferTo>::validate_handle(state, H::peel(handle), meta)
	}

	fn slot(&self) -> usize {
		H::peel_ref(self).slot()
	}
}

impl<T, H: Handle> Default for FreeList<T, H> {
	fn default() -> Self {
		Self {
			slots: Default::default(),
			free: Default::default(),
			state: H::default_state(),
		}
	}
}

impl<T, H: Handle> FreeList<T, H> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(&mut self, value: T) -> (&mut T, H) {
		if let Some(slot) = self.free.pop() {
			let slot_data = &mut self.slots[slot];

			// Generate a handle and slot metadata and register the slot in the state mirror
			let (handle, meta) = H::slot_occupied(&mut self.state, slot);

			// Insert the slot
			let slot_data = slot_data.insert((value, meta));

			(&mut slot_data.0, handle)
		} else {
			let slot = self.slots.len();

			// Generate a handle and slot metadata and register the slot in the state mirror
			let (handle, meta) = H::slot_occupied(&mut self.state, slot);

			// Insert the slot
			self.slots.push(Some((value, meta)));
			let slot_data = self.slots[slot].as_mut().unwrap();

			(&mut slot_data.0, handle)
		}
	}

	pub fn try_remove(&mut self, handle: H) -> Option<T> {
		// Fetch the slot
		let slot = handle.slot();
		let slot_data = self.slots.get_mut(slot)?;

		// Run custom handle validation
		if !H::validate_handle(&self.state, handle, &slot_data.as_ref()?.1) {
			return None;
		}

		// Ensure that the underlying slot also isn't empty, taking the value otherwise
		let (value, meta) = slot_data.take().unwrap();

		// Run custom handle de-registration
		H::slot_freed(&mut self.state, handle, meta);

		// Add to the free list
		// TODO: see if we can shrink the slot store
		self.free.push(slot);

		Some(value)
	}

	pub fn remove(&mut self, handle: H) -> T {
		self.try_remove(handle)
			.unwrap_or_else(|| panic!("FreeList does not contain element with handle {handle:?}."))
	}

	pub fn try_get(&self, handle: H) -> Option<&T> {
		// Fetch the slot
		let slot = handle.slot();
		let slot_data = self.slots.get(slot)?;
		let (value, meta) = slot_data.as_ref()?;

		// Run custom handle validation
		if !H::validate_handle(&self.state, handle, meta) {
			return None;
		}

		Some(value)
	}

	pub fn try_get_mut(&mut self, handle: H) -> Option<&mut T> {
		// Fetch the slot
		let slot = handle.slot();
		let slot_data = self.slots.get_mut(slot)?;
		let (value, meta) = slot_data.as_mut()?;

		// Run custom handle validation
		if !H::validate_handle(&self.state, handle, meta) {
			return None;
		}

		Some(value)
	}

	pub fn get(&self, handle: H) -> &T {
		self.try_get(handle)
			.unwrap_or_else(|| panic!("FreeList does not contain element with handle {handle:?}."))
	}

	pub fn get_mut(&mut self, handle: H) -> &mut T {
		self.try_get_mut(handle)
			.unwrap_or_else(|| panic!("FreeList does not contain element with handle {handle:?}."))
	}

	pub fn slots(&self) -> &[Option<(T, H::Meta)>] {
		&self.slots
	}

	pub fn slots_mut(&mut self) -> &mut [Option<(T, H::Meta)>] {
		&mut self.slots
	}
}

// === PureHandle === //

pub type PureFreeList<T> = FreeList<T, u32>;

macro_rules! impl_pure_handle_for {
	($const_new:ident; $($ty:ty),*) => {$(
		impl<T> FreeList<T, $ty> {
			pub const fn $const_new() -> Self {
				Self {
					slots: Vec::new(),
					free: Vec::new(),
					state: (),
				}
			}
		}

		impl Handle for $ty {
			type State = ();
			type Meta = ();

			fn default_state() -> Self::State {
				()
			}

			fn slot_occupied(_state: &mut Self::State, slot: usize) -> (Self, Self::Meta) {
				(
					<$ty>::try_from(slot).expect("allocated too many handles concurrently"),
					(),
				)
			}

			fn slot_freed(_state: &mut Self::State, _handle: Self, _meta: Self::Meta) {}

			fn validate_handle(_state: &Self::State, _handle: Self, _meta: &Self::Meta) -> bool {
				true
			}

			fn slot(&self) -> usize {
				*self as usize
			}
		}
	)*};
}

impl_pure_handle_for!(const_new; u32, u16, u8, usize);

// === FreeListHandle === //

#[allow(dead_code)]
mod debug_impl {
	use super::*;
	use std::num::NonZeroU64;

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub struct FreeListHandle {
		slot: usize,
		gen: NonZeroU64,
	}

	impl Handle for FreeListHandle {
		type State = NonZeroU64;
		type Meta = NonZeroU64;

		fn default_state() -> Self::State {
			NonZeroU64::new(1).unwrap()
		}

		fn slot_occupied(state: &mut Self::State, slot: usize) -> (Self, Self::Meta) {
			let gen = state
				.checked_add(1)
				.expect("allocated too many free list entries");

			(Self { slot, gen }, gen)
		}

		fn slot_freed(_state: &mut Self::State, _handle: Self, _meta: Self::Meta) {}

		fn validate_handle(_state: &Self::State, handle: Self, meta: &Self::Meta) -> bool {
			assert_eq!(handle.gen, *meta);
			true
		}

		fn slot(&self) -> usize {
			self.slot
		}
	}
}

#[allow(dead_code)]
mod release_impl {
	use super::*;

	#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
	pub struct FreeListHandle(usize);

	impl Handle for FreeListHandle {
		type State = ();
		type Meta = ();

		fn default_state() -> Self::State {
			()
		}

		fn slot_occupied(_state: &mut Self::State, slot: usize) -> (Self, Self::Meta) {
			(Self(slot), ())
		}

		fn slot_freed(_state: &mut Self::State, _handle: Self, _meta: Self::Meta) {}

		fn validate_handle(_state: &Self::State, _handle: Self, _meta: &Self::Meta) -> bool {
			true
		}

		fn slot(&self) -> usize {
			self.0
		}
	}
}

#[cfg(debug_assertions)]
pub use debug_impl::*;

#[cfg(not(debug_assertions))]
pub use release_impl::*;
