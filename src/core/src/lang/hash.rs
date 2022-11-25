use std::hash::{self, BuildHasherDefault};

use crate::debug::type_id::are_probably_equal;

pub type NoOpBuildHasher = BuildHasherDefault<NoOpHasher>;

#[derive(Debug, Clone, Default)]
pub struct NoOpHasher(u64);

impl hash::Hasher for NoOpHasher {
	fn write_u32(&mut self, i: u32) {
		debug_assert_eq!(self.0, 0);
		let i = i as u64;
		self.0 = (i << 32) + i;
	}

	fn write_u64(&mut self, i: u64) {
		debug_assert_eq!(self.0, 0);
		self.0 = i;
	}

	fn write(&mut self, _bytes: &[u8]) {
		unimplemented!("NoOpHasher only supports `write_u64` and `write_u32`.");
	}

	fn finish(&self) -> u64 {
		self.0
	}
}

#[derive(Debug, Copy, Clone)]
pub struct PreHashed<T> {
	pub hash: u64,
	pub value: T,
}

impl<T: Eq> Eq for PreHashed<T> {}

impl<T: PartialEq> PartialEq for PreHashed<T> {
	fn eq(&self, other: &Self) -> bool {
		self.value == other.value
	}
}

impl<T> hash::Hash for PreHashed<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		assert!(are_probably_equal::<NoOpHasher, H>());

		state.write_u64(self.hash);
	}
}
