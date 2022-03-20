use derive_where::derive_where;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;

// === Internal modules === //

mod arch;
mod entities;
mod queue;

// === Identifiers === //

// TODO: Use non-zero types for these identifiers to allow for better niche optimizations.

/// An entity generation; used to distinguish between multiple distinct entities in a single slot.
pub type EntityGen = u64;
type AtomicEntityGen = AtomicU64;

/// The unique identifier of a storage.
type StorageId = u64;
type AtomicStorageId = AtomicU64;

/// An archetype generation; used to distinguish between multiple distinct archetypes in a single slot.
pub type ArchGen = u64;

/// An identifier for a snapshot in the archetype's history. Used to lazily bring storages up-to-date.
pub type DirtyId = u64;

// === World === //

#[derive(Debug)]
pub struct World;

// === Entity === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity<A = ()> {
	_ty: PhantomData<fn(A) -> A>,
	index: usize,
	gen: EntityGen,
}
