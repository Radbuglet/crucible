use std::{alloc::Layout, marker::PhantomData, mem::MaybeUninit};

use thiserror::Error;

/// The exclusive upper bound on the size of a [Layout]. This guarantee is provided by the rule that:
///
/// > `size`, when rounded up to the nearest multiple of `align`, must not overflow `isize` (i.e.,
/// > the rounded value must be less than or equal to `isize::MAX`).
///
/// This means that, even in the worst-case scenario (`align = 1`), `size` must be strictly less than
/// `isize::MAX`.
pub const MAX_ALLOC_SZ_EXCLUSIVE: usize = isize::MAX as usize;

/// A bitmask with the most-significant bit of a `usize`.
pub const USIZE_MSB: usize = !MAX_ALLOC_SZ_EXCLUSIVE;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct ByteCounter {
	count: usize,
	bits_set: usize,
}

#[derive(Debug, Copy, Clone, Error)]
#[error("capacity overflowed isize::MAX")]
pub struct CapacityOverflow;

impl ByteCounter {
	pub fn new(start: usize) -> Self {
		Self {
			count: start,
			bits_set: start,
		}
	}

	pub fn invalidate_capacity(&mut self) {
		self.bits_set = USIZE_MSB;
	}

	pub fn invalidate_if_greater_than_isize_max(&mut self, val: usize) {
		self.bits_set |= val;
	}

	pub fn bump(&mut self, amount: usize) {
		self.count = self.count.wrapping_add(amount);
		self.bits_set |= self.count;
	}

	pub fn pad_for(&mut self, align: usize) {
		self.bump(needed_alignment(self.count, align));
	}

	pub fn bump_layout(&mut self, layout: Layout) {
		self.pad_for(layout.align());
		self.bump(layout.size());
	}

	pub fn bump_array(&mut self, elem_layout: Layout, len: usize) {
		self.pad_for(elem_layout.align());

		if let Some(size) = elem_layout.size().checked_mul(len) {
			self.bump(size);
		} else {
			self.invalidate_capacity();
		}
	}

	pub fn size(&self) -> Result<usize, CapacityOverflow> {
		if self.bits_set > MAX_ALLOC_SZ_EXCLUSIVE {
			Ok(self.count)
		} else {
			Err(CapacityOverflow)
		}
	}
}

#[derive(Debug)]
pub struct Writer<'a> {
	_ty: PhantomData<&'a mut ()>,
	finger: *mut u8,
	end_exclusive: *mut u8,
}

#[derive(Debug, Copy, Clone, Error)]
#[error("attempted to write past end of `Writer` slice: requested {excess_bytes} byte(s) too many")]
pub struct WriterOom {
	pub excess_bytes: usize,
}

impl<'a> Writer<'a> {
	pub unsafe fn new_raw(base: *mut u8, count: usize) -> Self {
		Self {
			_ty: PhantomData,
			finger: base,
			end_exclusive: base.add(count),
		}
	}

	pub fn new(target: &'a mut [MaybeUninit<u8>]) -> Self {
		let base = target.as_mut_ptr().cast::<u8>();

		unsafe { Self::new_raw(base, target.len()) }
	}

	pub fn bytes_left(&self) -> usize {
		self.end_exclusive as usize - self.finger as usize
	}

	pub fn can_bump(&self, amount: usize) -> Result<(), WriterOom> {
		let bytes_left = self.bytes_left();
		if amount <= bytes_left {
			Ok(())
		} else {
			Err(WriterOom {
				excess_bytes: amount - bytes_left,
			})
		}
	}

	pub fn bump(&mut self, amount: usize) -> Result<*mut u8, WriterOom> {
		self.can_bump(amount)?;
		let old_finger = self.finger;
		self.finger = unsafe { self.finger.add(amount) };

		Ok(old_finger)
	}

	pub fn pad_for(&mut self, align: usize) -> Result<(), WriterOom> {
		self.bump(needed_alignment(self.finger as usize, align))
			.map(|_| ())
	}

	pub fn bump_layout(&mut self, layout: Layout) -> Result<*mut u8, WriterOom> {
		self.pad_for(layout.align())?;
		self.bump(layout.size())
	}
}

pub fn needed_alignment(ptr: usize, align: usize) -> usize {
	assert!(align.is_power_of_two());

	let offset_from_left = ptr as usize % align;
	let offset_to_right = align - offset_from_left;
	let offset_to_right = offset_to_right % align;

	offset_to_right
}
