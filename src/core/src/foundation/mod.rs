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
//! inside a [Provider]. Subsystems are accessed by their component type.
//!
//! Scheduling is done entirely through `async` functions executed by the engine's [Executor] with
//! [RwLock] objects mediating resource access. Users can dynamically add futures to the [DynJoinFuture]
//! to dynamically define task barriers.
//!
//! Because locks are coarse-grained and long-lived, simple callbacks are insufficient to handle
//! event hooking, and users must typically defer event handling to fixed points in the execution
//! pipeline. There are many ways to implement this, each with their own performance characteristics.
//! To allow users to choose the exact dispatch mechanism, we provide the [EventPusher] trait
//! alongside [various event pushing strategies](event_pushing).
//!
//! Entities are handled by the [hecs] crate, with the [WorldWrapper] singleton handling deferred
//! entity creation/deletion and component locking.

pub use hecs;

pub mod event;
pub mod exec;
pub mod lock;
pub mod provider;
pub mod world;
