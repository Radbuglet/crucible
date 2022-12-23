use std::hash::{self, BuildHasherDefault};

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
