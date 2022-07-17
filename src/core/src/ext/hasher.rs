use std::hash::{BuildHasher, Hash, Hasher};

pub fn hash_one<B: ?Sized + BuildHasher, H: ?Sized + Hash>(builder: &B, target: &H) -> u64 {
	let mut hasher = builder.build_hasher();
	target.hash(&mut hasher);
	hasher.finish()
}
