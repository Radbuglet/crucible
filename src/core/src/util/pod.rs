//! A manual plain-old-data (Pod) writing system because using compiler-generated struct layouts with
//! non-zero padding causes undefined behavior when casting to a byte array and uploading to the GPU.

use crate::util::pointer::{align_up_offset, layout_from_size_and_align};
use std::alloc::Layout;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::mem::MaybeUninit;

// === Core Writer === //

/// An object capable of writing bytes to a buffer sequentially with resizing/size limiting capacity
/// strategies.
///
/// ## Safety
///
/// Method semantics for every method must be fulfilled.
///
pub unsafe trait PodWriter {
	type GrowError: Error;

	// === Core methods === //

	/// Returns the offset from the beginning of the buffer. Used to determine alignment offsets.
	fn location(&self) -> usize;

	/// Returns the number of bytes which can be safely requested through [request_unchecked] before
	/// the buffer needs to request more memory.
	fn space_left(&self) -> usize;

	/// Ensures that the space left is at least `min_left`, attempting to grow the writer if not. This
	/// guarantee is only made on successful returns.
	fn try_ensure_space(&mut self, min_space: usize) -> Result<(), Self::GrowError>;

	/// Ensures that the space left is at least `min_left`, attempting to grow the writer if not. This
	/// guarantee is only made on successful returns, with the method panicking if space could not be
	/// reserved.
	fn ensure_space(&mut self, min_space: usize) {
		self.try_ensure_space(min_space).unwrap();
	}

	/// Requests exactly `size` bytes at the head of the buffer (no padding added) to be initialized
	/// by the caller without checking capacity.
	///
	/// ## Safety
	///
	/// - The capacity of the buffer must be at least `size` bytes at the time of calling.
	/// - The entire slice must be initialized before calling other methods on the object.
	///
	unsafe fn request_unchecked(&mut self, size: usize) -> &mut [MaybeUninit<u8>];

	/// Requests exactly `size` bytes at the head of the buffer (no padding added) to be initialized
	/// by the caller, attempting to grow if the capacity is insufficient to accommodate the request,
	/// and returning `Err` if the request failed.
	///
	/// ## Safety
	///
	/// - The entire slice must be initialized before calling other methods on the object.
	///
	unsafe fn try_request(
		&mut self,
		size: usize,
	) -> Result<&mut [MaybeUninit<u8>], Self::GrowError> {
		self.try_ensure_space(size)?;
		// Safety: init guarantees provided by caller; capacity is checked above.
		Ok(self.request_unchecked(size))
	}

	/// Requests exactly `size` bytes at the head of the buffer (no padding added) to be initialized
	/// by the caller, attempting to grow if the capacity is insufficient to accommodate the request,
	/// and panicking if the request failed.
	///
	/// ## Safety
	///
	/// - The entire slice must be initialized before calling other methods on the object.
	///
	unsafe fn request(&mut self, size: usize) -> &mut [MaybeUninit<u8>] {
		// Safety: init guarantees provided by caller
		self.try_request(size).unwrap()
	}

	// === Shortcut methods === //

	fn try_write<T: ?Sized + PodSerializable>(
		&mut self,
		target: &T,
	) -> Result<usize, Self::GrowError> {
		target.try_write(self)
	}

	fn write<T: ?Sized + PodSerializable>(&mut self, target: &T) -> usize {
		self.try_write(target).unwrap()
	}

	fn try_skip(&mut self, bytes: usize) -> Result<usize, Self::GrowError> {
		self.try_write(&Padding(bytes))
	}

	fn skip(&mut self, bytes: usize) -> usize {
		self.try_skip(bytes).unwrap()
	}

	fn try_align_to(&mut self, align: usize) -> Result<usize, Self::GrowError> {
		self.try_skip(align_up_offset(self.location(), align))
	}

	fn align_to(&mut self, align: usize) -> usize {
		self.try_align_to(align).unwrap()
	}

	// === Higher level utilities === //

	unsafe fn wrap_unchecked(&mut self) -> UncheckedPodWriter<'_, Self> {
		// Safety: provided by caller
		UncheckedPodWriter::new(self)
	}

	fn try_new_sub(&mut self, layout: Layout) -> Result<PodSubWriter<'_, Self>, Self::GrowError> {
		// Determine alignment padding
		let padding = align_up_offset(self.location(), layout.align());

		// Determine total reserved capacity
		let reserved = layout.size().checked_add(padding).unwrap();

		// Create a writer, growing if required
		let mut writer = PodSubWriter::new_can_try_grow(self, reserved)?;

		// Insert left padding. It is up to users to insert right padding, if desired.
		writer.skip(padding);

		// Let users fill out the rest!
		Ok(writer)
	}

	fn new_sub(&mut self, layout: Layout) -> PodSubWriter<'_, Self> {
		self.try_new_sub(layout).unwrap()
	}
}

#[derive(Debug)]
pub struct UncheckedPodWriter<'a, O: ?Sized> {
	writer: &'a mut O,
}

impl<'a, O: ?Sized> UncheckedPodWriter<'a, O> {
	pub unsafe fn new(writer: &'a mut O) -> Self {
		Self { writer }
	}

	pub fn writer(&self) -> &O {
		&self.writer
	}

	pub fn writer_mut(&mut self) -> &mut O {
		&mut self.writer
	}

	pub fn into_writer(self) -> &'a mut O {
		self.writer
	}
}

unsafe impl<'a, O: ?Sized + PodWriter> PodWriter for UncheckedPodWriter<'a, O> {
	type GrowError = !;

	fn location(&self) -> usize {
		self.writer.location()
	}

	fn space_left(&self) -> usize {
		usize::MAX
	}

	fn try_ensure_space(&mut self, _min_space: usize) -> Result<(), Self::GrowError> {
		Ok(())
	}

	unsafe fn request_unchecked(&mut self, size: usize) -> &mut [MaybeUninit<u8>] {
		self.writer.request_unchecked(size)
	}
}

// === Writer adapters === //

#[derive(Debug)]
pub struct FixedSizeGrowError;

impl FixedSizeGrowError {
	pub fn result_for_reserve(min_space: usize, space_left: usize) -> Result<(), Self> {
		if min_space <= space_left {
			Ok(())
		} else {
			Err(FixedSizeGrowError)
		}
	}
}

impl Error for FixedSizeGrowError {}

impl Display for FixedSizeGrowError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "attempted to grow a fixed size buffer")
	}
}

#[derive(Debug)]
pub struct PodSubWriter<'a, O: ?Sized + PodWriter> {
	writer: &'a mut O,
	remaining: usize,
}

impl<'a, O: ?Sized + PodWriter> PodSubWriter<'a, O> {
	pub unsafe fn new_unchecked(writer: &'a mut O, size: usize) -> Self {
		Self {
			writer,
			remaining: size,
		}
	}

	pub fn new_can_try_grow(writer: &'a mut O, size: usize) -> Result<Self, O::GrowError> {
		unsafe {
			writer.try_ensure_space(size)?;
			Ok(Self::new_unchecked(writer, size))
		}
	}

	pub fn new_can_grow(writer: &'a mut O, size: usize) -> Self {
		Self::new_can_try_grow(writer, size).unwrap()
	}

	pub fn skip_to_fill(&mut self) {
		self.skip(self.remaining);
	}
}

unsafe impl<'a, O: ?Sized + PodWriter> PodWriter for PodSubWriter<'a, O> {
	type GrowError = FixedSizeGrowError;

	fn location(&self) -> usize {
		self.writer.location()
	}

	fn space_left(&self) -> usize {
		self.remaining
	}

	fn try_ensure_space(&mut self, min_space: usize) -> Result<(), Self::GrowError> {
		FixedSizeGrowError::result_for_reserve(min_space, self.remaining)
	}

	unsafe fn request_unchecked(&mut self, size: usize) -> &mut [MaybeUninit<u8>] {
		self.remaining -= size;
		self.writer.request_unchecked(size)
	}
}

impl<'a, O: ?Sized + PodWriter> Drop for PodSubWriter<'a, O> {
	fn drop(&mut self) {
		#[cfg(debug_assertions)]
		if !std::thread::panicking() {
			debug_assert_eq!(
				self.remaining, 0,
				"PodSubWriter failed to write complete object: missing {} byte(s).",
				self.remaining
			);
		}
	}
}

// === Simple writers === //

#[derive(Debug, Clone, Default)]
pub struct VecWriter {
	pub bytes: Vec<u8>,
}

impl VecWriter {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_raw(bytes: Vec<u8>) -> Self {
		Self { bytes }
	}

	pub fn reset(&mut self) {
		self.bytes.clear();
	}

	pub fn bytes(&self) -> &[u8] {
		&self.bytes
	}
}

unsafe impl PodWriter for VecWriter {
	type GrowError = std::collections::TryReserveError;

	fn location(&self) -> usize {
		self.bytes.len()
	}

	fn space_left(&self) -> usize {
		self.bytes.capacity() - self.bytes.len()
	}

	fn try_ensure_space(&mut self, min_space: usize) -> Result<(), Self::GrowError> {
		self.bytes.try_reserve(min_space)
	}

	unsafe fn request_unchecked(&mut self, size: usize) -> &mut [MaybeUninit<u8>] {
		use std::slice::from_raw_parts_mut;

		let slice = from_raw_parts_mut(
			self.bytes
				.as_mut_ptr()
				.add(self.location())
				.cast::<MaybeUninit<u8>>(),
			size,
		);
		self.bytes.set_len(self.bytes.len() + size);
		slice
	}
}

#[derive(Debug)]
pub struct ArrayWriter<'a> {
	pub bytes: &'a mut [u8],
	pub head: usize,
}

impl<'a> ArrayWriter<'a> {
	pub fn new(bytes: &'a mut [u8]) -> Self {
		Self { bytes, head: 0 }
	}
}

unsafe impl PodWriter for ArrayWriter<'_> {
	type GrowError = FixedSizeGrowError;

	fn location(&self) -> usize {
		self.head
	}

	fn space_left(&self) -> usize {
		// We use `saturating_sub` to prevent out-of-bounds heads from causing U.B.
		self.bytes.len().saturating_sub(self.head)
	}

	fn try_ensure_space(&mut self, min_space: usize) -> Result<(), Self::GrowError> {
		FixedSizeGrowError::result_for_reserve(min_space, self.space_left())
	}

	unsafe fn request_unchecked(&mut self, size: usize) -> &mut [MaybeUninit<u8>] {
		use std::slice::from_raw_parts_mut;

		let slice = from_raw_parts_mut(
			self.bytes
				.as_mut_ptr()
				.cast::<MaybeUninit<u8>>()
				.add(self.head),
			size,
		);
		self.head += size;
		slice
	}
}

// === Core Pod === //

pub trait PodSerializable {
	fn try_write<T: ?Sized + PodWriter>(&self, writer: &mut T) -> Result<usize, T::GrowError>;
}

impl PodSerializable for [u8] {
	fn try_write<T: ?Sized + PodWriter>(&self, writer: &mut T) -> Result<usize, T::GrowError> {
		let head = writer.location();
		unsafe {
			MaybeUninit::write_slice(&mut writer.try_request(self.len())?, self);
		}
		Ok(head)
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Padding(pub usize);

impl PodSerializable for Padding {
	fn try_write<T: ?Sized + PodWriter>(&self, writer: &mut T) -> Result<usize, T::GrowError> {
		let head = writer.location();
		unsafe {
			let slice = writer.try_request(self.0)?;
			slice.as_mut_ptr().write_bytes(0xFF, self.0);
		}
		Ok(head)
	}
}

pub trait FixedPodSerializable {
	const LAYOUT: Layout;

	fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>);
}

impl<O: FixedPodSerializable> PodSerializable for O {
	fn try_write<T: ?Sized + PodWriter>(&self, writer: &mut T) -> Result<usize, T::GrowError> {
		let mut sub = writer.try_new_sub(O::LAYOUT)?;
		let head = sub.location();
		self.write(&mut sub);
		Ok(head)
	}
}

pub fn size_of_pod<T: FixedPodSerializable>() -> usize {
	T::LAYOUT.size()
}

pub fn align_of_pod<T: FixedPodSerializable>() -> usize {
	T::LAYOUT.align()
}

// TODO: Use const arrays
pub fn bytes_of_pod<T: FixedPodSerializable>(value: &T) -> Vec<u8> {
	let mut writer = VecWriter::from_raw(Vec::with_capacity(size_of_pod::<T>()));
	writer.write(value);
	writer.bytes
}

// === Pod Macros === //

#[allow(unused_imports)] // For use in the macro.
use crate::util::wrapper::Wrapper;

//noinspection DuplicatedCode
pub macro pod_struct {
	// Base case
	() => {},

	// Dynamically-sized structs
	(
		$(#[$item_attr:meta])*
		$struct_vis:vis struct $struct_name:ident {
			$(
				$(#[$field_attr:meta])*
				$field_vis:vis $field_name:ident: $field_ty:ty $([$($wrapper:tt)*])?
			),*
			$(,)?
		}

		$($remaining:tt)*
	) => {
		// Define struct
		$(#[$item_attr])*
		$struct_vis struct $struct_name {
			$(
				$(#[$field_attr])*
				$field_vis $field_name: $field_ty
			),*
		}

		// Implement serializer
		impl PodSerializable for $struct_name {
			fn try_write<T: ?Sized + PodWriter>(&self, writer: &mut T) -> Result<usize, T::ReserveError> {
				$({
					let wrap = __internal_make_identity_closure::<$field_ty>();
					$(let wrap = $($wrapper)*::<$field_ty>::from_ref;)?

					writer.try_write(wrap(&self.$field_name))?;
				})*
				Ok(())
			}
		}

		// Munch remaining
		pod_struct!($($remaining)*);
	},

	// Fixed-size structs
	(
		$(#[$item_attr:meta])*
		$struct_vis:vis fixed struct $struct_name:ident {
			$(
				$(#[$field_attr:meta])*
				$field_vis:vis $field_name:ident: $field_ty:ty $([$($wrapper:tt)*])?
			),*
			$(,)?
		}

		$($remaining:tt)*
	) => {
		// Define struct
		$(#[$item_attr])*
		$struct_vis struct $struct_name {
			$(
				$(#[$field_attr])*
				$field_vis $field_name: $field_ty
			),*
		}

		// Implement serializer
		impl FixedPodSerializable for $struct_name {
			const LAYOUT: Layout = {
				let layout = Layout::new::<()>();

				$(
					let layout = {
						type FieldTy =
							// If wrapper...
							$($($wrapper)* <$field_ty>; #[allow(unused)] type Discard =)?
							// Fallback
							$field_ty;

						__internal_const_extend(layout, <FieldTy as FixedPodSerializable>::LAYOUT).0
					};
				)*

				__internal_const_pad_to_align(layout)
			};

			fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
				$({
					let wrap = __internal_make_identity_closure::<$field_ty>();
					$(let wrap = $($wrapper)*::<$field_ty>::from_ref;)?

					writer.write(wrap(&self.$field_name));
				})*
				writer.skip_to_fill();
			}
		}

		// Munch remaining
		pod_struct!($($remaining)*);
	},
}

// TODO: Remove once "Layout::extend" gets const-fn stabilized.
#[doc(hidden)]
pub const fn __internal_const_extend(layout: Layout, next: Layout) -> (Layout, usize) {
	const fn const_max(lhs: usize, rhs: usize) -> usize {
		if lhs > rhs {
			lhs
		} else {
			rhs
		}
	}

	const fn check_add(lhs: usize, rhs: usize) -> usize {
		match lhs.checked_add(rhs) {
			Some(val) => val,
			None => panic!("Overflowed when extending pod layout."),
		}
	}

	let new_align = const_max(layout.align(), next.align());
	let pad = layout.padding_needed_for(next.align());

	let offset = check_add(layout.size(), pad);
	let new_size = check_add(offset, next.size());

	let layout = layout_from_size_and_align(new_size, new_align);
	(layout, offset)
}

#[doc(hidden)]
pub const fn __internal_const_pad_to_align(layout: Layout) -> Layout {
	let pad = layout.padding_needed_for(layout.align());
	// This cannot overflow. Quoting from the invariant of Layout:
	// > `size`, when rounded up to the nearest multiple of `align`,
	// > must not overflow (i.e., the rounded value must be less than
	// > `usize::MAX`)
	let new_size = layout.size() + pad;

	layout_from_size_and_align(new_size, layout.align())
}

#[doc(hidden)]
pub fn __internal_make_identity_closure<T: ?Sized>() -> impl for<'a> FnOnce(&'a T) -> &'a T {
	|x| x
}
