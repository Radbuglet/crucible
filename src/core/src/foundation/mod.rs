//! The module implementing Crucible's core engine infrastructure.
//!
//! ## Architecture
//!
//! Crucible's low-level backend is built as a collection of subsystem monoliths, where each
//! subsystem has complete ownership over its internal state at all levels of the entity hierarchy.
//! This was done to minimize the number of synchronization locks required during execution but
//! otherwise provides no architectural benefits over the userland object-oriented model.
//!
//! All subsystems, including special "core" singletons such as the executor and the world, are stored
//! inside a [Provider]. Subsystems are accessed by their component type. Providers are mainly used
//! to facilitate passing the engine's state between threads and to simplify dependency tuple packing.
//! Services should always attempt to request the most primitive version of a dependency reference
//! (e.g. preferring a direct reference over an entire [RwLock] instance) so that lock lifetimes can
//! be handled externally.
//!
//! Scheduling is done entirely through `async` functions executed by the engine's [Executor] with
//! [RwLock] objects mediating resource access. Users can dynamically add futures to the [DynJoinFuture]
//! to dynamically define task barriers.
//!
//! Because locks are coarse-grained and long-lived, simple callbacks are insufficient to handle
//! event hooking, and users must typically defer event handling to fixed points in the execution
//! pipeline after the dependency locks have been released. There are many ways to implement this,
//! each with their own performance characteristics. To allow users to choose the exact dispatch
//! mechanism, we provide the [EventPusher] trait alongside [various event pushing strategies](event).
//!
//! Entities are handled by a simplistic parallel ECS implementation residing in [world].

pub mod event;
pub mod exec;
pub mod ext;
pub mod lock;
pub mod provider;
pub mod world;

pub mod prelude {
	pub use super::{
		event::{EventPusher, EventPusherImmediate, EventPusherPoll},
		exec::{DynJoinFuture, Executor},
		ext::ProviderRwLockExt,
		lock::{RwGuard, RwGuardMut, RwGuardRef, RwLock, RwLockManager},
		provider::{
			Component, LazyComponent, LazyProviderExt, MultiProvider, Provider, ProviderExt,
		},
		world::{Entity, MapStorage, World},
	};
}

pub use prelude::*;
