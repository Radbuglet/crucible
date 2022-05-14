use std::alloc::Layout;
use std::any::type_name;
use std::mem::ManuallyDrop;
use thiserror::Error;

#[derive(Copy, Clone)]
pub union ByteContainer<H> {
	zst: (),
	_never: ManuallyDrop<H>,
}

impl<H> ByteContainer<H> {
	pub fn can_host<G>() -> Result<(), ByteContainerError> {
		let guest_layout = Layout::new::<G>();
		let host_layout = Layout::new::<H>();

		if guest_layout.size() > host_layout.size() {
			return Err(ByteContainerError::Size {
				guest_name: type_name::<G>(),
				guest_size: guest_layout.size(),
				host_name: type_name::<H>(),
				host_size: host_layout.size(),
			});
		}

		// We're comparing powers of two so this properly checks for factorizations.
		if guest_layout.align() > host_layout.align() {
			return Err(ByteContainerError::Align {
				guest_name: type_name::<G>(),
				guest_align: guest_layout.size(),
				host_name: type_name::<H>(),
				host_align: host_layout.size(),
			});
		}

		Ok(())
	}

	pub fn try_new<G>(value: G) -> Result<Self, (G, ByteContainerError)> {
		// Validate layout
		if let Err(err) = Self::can_host::<G>() {
			return Err((value, err));
		}

		// Construct object
		let mut target = Self { zst: () };
		unsafe {
			// Use the extra padding bytes to store `T`. This behavior is guaranteed by the Rust
			// [Unsafe Code Guidelines](UCG), which promises that a union containing a ZST can house
			// any bit pattern.
			// [UCG]: https://rust-lang.github.io/unsafe-code-guidelines/validity/unions.html#validity-of-unions-with-zero-sized-fields
			(&mut target as *mut Self).cast::<G>().write(value);
		}

		Ok(target)
	}

	pub fn new<G>(value: G) -> Self {
		Self::try_new(value).map_err(|(_, err)| err).unwrap()
	}

	pub fn as_const_ptr<G>(&self) -> *const G {
		#[cfg(debug_assertions)]
		Self::can_host::<G>().unwrap();
		(self as *const Self).cast::<G>()
	}

	pub unsafe fn as_ref<G>(&self) -> &G {
		&*self.as_const_ptr()
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Error)]
pub enum ByteContainerError {
	#[error(
		"Guest value of type {guest_name} and size {guest_size} bytes is too much for the host \
	         of type {host_name}, which can store at most {host_size} byte(s)!"
	)]
	Size {
		guest_name: &'static str,
		guest_size: usize,
		host_name: &'static str,
		host_size: usize,
	},
	#[error(
		"Guest value of type {guest_name} and alignment {guest_align} bytes is too constricting \
	         for the host of type {host_name}, which can align to at most {host_align} byte(s)!"
	)]
	Align {
		guest_name: &'static str,
		guest_align: usize,
		host_name: &'static str,
		host_align: usize,
	},
}
