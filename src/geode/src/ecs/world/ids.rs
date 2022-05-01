use crate::util::number::NonZeroU64Generator;
use std::num::NonZeroU64;

/// An entity generation; used to distinguish between multiple distinct entities in a single slot.
pub type EntityGen = NonZeroU64;
pub type EntityGenGenerator = NonZeroU64Generator;

/// An entity generation; used to distinguish between multiple distinct entities in a single slot.
pub type StorageId = NonZeroU64;
pub type StorageIdGenerator = NonZeroU64Generator;

/// An archetype generation; used to distinguish between multiple distinct archetypes in a single slot.
pub type ArchGen = NonZeroU64;

/// An identifier for a snapshot in the archetype's history. Used to lazily bring storages up-to-date.
pub type DirtyId = NonZeroU64;
