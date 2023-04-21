use std::{hash, marker::PhantomData};

use crate::lang::marker::PhantomShorten;

// === ConstSafeBuildHasherDefault === //

pub struct ConstSafeBuildHasherDefault<T>(PhantomShorten<T>);

impl<T> ConstSafeBuildHasherDefault<T> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T: Default + hash::Hasher> hash::BuildHasher for ConstSafeBuildHasherDefault<T> {
	type Hasher = T;

	fn build_hasher(&self) -> Self::Hasher {
		T::default()
	}
}

impl<T> Default for ConstSafeBuildHasherDefault<T> {
	fn default() -> Self {
		Self::new()
	}
}

// === Hash Maps === //

pub type FxHashBuilder = ConstSafeBuildHasherDefault<fxhash::FxHasher>;
pub type FxHashMap<K, V> = hashbrown::HashMap<K, V, FxHashBuilder>;
pub type FxHashSet<T> = hashbrown::HashSet<T, FxHashBuilder>;
