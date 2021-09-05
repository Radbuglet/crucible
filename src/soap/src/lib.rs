//! Crucible's low-level backend is built as a collection of subsystem monoliths, where each
//! subsystem has complete ownership over its internal state at all levels of the entity hierarchy.
//! This was done to minimize the number of synchronization locks required during execution but
//! otherwise provides no architectural benefits over the object-oriented userland model.
//!
//! The root of all the monoliths is the [Engine] singleton, a context object providing the
//! scheduling, entity management, and subsystem-fetching components of this pattern.
//!
//! Scheduling is done entirely through `async` functions executed by the engine's [Executor] with
//! [RwLock] objects handling resource contention. There are many ways to dynamically dispatch event
//! handlers in an ECS-like pattern, each with their own performance characteristics. To allow users
//! to choose the exact dispatch mechanism, we provide the [EventPusher] trait alongside
//! [various event pushing strategies](event_pushers).
//!
//! Entities are handled by [hecs], with the additional [WorldCommandBuffer] handling deferred entity
//! creation and management.
//!
//! Every subsystem, also called a "server", is registered in the [Servers] container and can be fetched
//! based off its type. This allows us to decouple servers from their execution environment by providing
//! dependencies through an abstract resource list instead of through a specific engine layout.

pub use hecs;

pub struct Engine {
	pub executor: Executor,
	pub servers: Servers,
	pub world: WorldCommandBuffer,
}

pub struct Executor {}

pub struct RwLock {}

pub struct DynTaskPool {}

pub trait EventPusher {}

pub mod event_pushers {
	pub struct TagEventPusher {}

	pub struct ScheduleEventPusher {}

	pub struct FnEventPusher {}
}

pub struct WorldCommandBuffer {}

pub struct Servers {}
