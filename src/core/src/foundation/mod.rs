//! The module implementing Crucible's core engine infrastructure.
//!
//! ## Overview
//!
//! Crucible's low-level architecture is designed under one central constraint: Rust's mutability
//! rules. Unlike many other languages, Rust does not allow zero-cost multithreaded shared ownership
//! which makes minimizing runtime borrow checks imperative to high performance.
//!
//! Thus, unlike Crucible's userland, the data structures are modeled more off memory access patterns
//! than off the logical hierarchy of the game. To enable this form of development while providing the
//! same extensibility and convenience of the object-oriented solution, a few core utilities are
//! provided:
//!
//! One of the most essential developments in modern game architecture is the focus on composable
//! entities. There are two systems to achieve this: type [Provider]s and entity [Storage]s.
//! [Provider]s represent a single logical entity and map *types* to components. [Storage]s, on the
//! other hand, represent a *collection of component instances*, and map entities to components.
//!
//! [Storage]s are heavily inspired from the entity-component portion of the [ECS pattern] but are
//! arguably more powerful. Instead of storing all storages in a global resource container and
//! allowing arbitrary "systems" to access them, [Storage]s are be owned by individual objects who
//! mediate access to their state in a safe manner. There can be multiple storages for each component
//! type and components can store their own storages, enabling users to create deeply nested structures.
//! [Storage]s can be wrapped to track ownership semantics at runtime, to split the storage into
//! several sub-storages, or to change the way in which a consumer can access the underlying data.
//!
//! [Provider]s, while less commonly used than [Storage]s, are an essential part of this programming
//! model. Unlike components stored in a [Storage], components in a [Provider] can be fetched by
//! their type alone. This makes them useful for signal dispatch, where an arbitrary number of
//! consumers may require different portions of a given service (e.g. the main loop may only need the
//! graphics singleton of the engine root).
//!
//! Back-references to logical parents are discouraged in this architecture because they would require
//! borrow patterns to be checked at runtime with an `Arc<Mutex<T>>` pair. Instead, consumers are
//! expected to pass all their dependencies in their most primitive forms (e.g. preferring `&mut T`
//! over `RwGuardMut<T>`), wrapping and splitting them as needed.
//!
//! To avoid execution serialization, dynamic dispatch, and to keep query loops efficient, components
//! will oftentimes emit events which may need to be deferred. There are many ways of handling events,
//! each with their own performance characteristics, so it is best for abstractions to leave the event
//! handling strategy up to the consumer. This is done by taking in an object implementing the
//! [EventPusher] trait. In addition to this trait, the [event] module provides a few standard event-
//! pushing strategies (e.g. immediate, single-threaded and multi-threaded dequeues, iterator optimized
//! dequeues, and storage tagging).
//!
//! Scheduling in this model is handled through `async` functions. The details of this system are
//! still being decided.
//! TODO: Introduce signals, executors, and locks. Tie in with `Providers`.
//! TODO: We might also want to inline dyn values for the strategy pattern.
//!
//! ## Justifying Flexibility
//!
//! This architecture can seem quite foreign to many. Because Rust's borrow checker prevents "circling
//! back" to an ancestor instance in the execution stack, the update loop can feel very much like one
//! giant "map" function where each object maps their (and their descendants') state to the next
//! frame's state, producing events or using external borrowing mechanisms to manipulate siblings and
//! ancestor objects. In a sense, Rust encourages similar game architecture patterns to those seen in
//! functional languages— although interior mutability's escape hatch provides room for more
//! traditional implementations.
//!
//! Reconciling why this model works can be quite difficult, especially because many object oriented
//! techniques do not have a direct analog. For example, it is basically impossible to store a direct
//! reference to a specific component without wrapping either the `Storage` or the `Component` with
//! a heavyweight `Arc<Mutex<...>>` object. This can be uncomfortable, as it implies that the optimal
//! implementation may be prevented by sticking too closely to idiomatic Rust architecture.
//!
//! TODO: This pattern as a feature-complete generalized form of ECS.

pub mod event;
pub mod exec;
pub mod ext;
pub mod lock;
pub mod provider;
pub mod world;

pub mod prelude {
	pub use super::{
		event::{EventPusher, EventPusherCallback, EventPusherImmediate},
		exec::Executor,
		ext::{ProviderRwLockExt, RwLockComponent},
		lock::{lock_many_now, RwGuard, RwGuardMut, RwGuardRef, RwLock, RwLockManager},
		provider::{
			get_many, Component, LazyComponent, LazyProviderExt, MultiProvider, Provider,
			ProviderExt,
		},
		world::{Entity, Storage, World},
	};
}

pub use prelude::*;
