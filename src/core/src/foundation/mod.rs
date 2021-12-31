//! The module implementing Crucible's core engine infrastructure.
//!
//! ## Background
//!
//! Crucible's architecture is largely impacted by one central constraint: the borrow checker. In
//! order for the borrow checker to provide useful results, users are only capable of borrowing an
//! object mutably (without resorting to heavyweight synchronization primitives such as
//! `Arc<Mutex<...>>`) once. In Rust, application state forms a strict tree where objects needing
//! mutation can never contain back-references to their ancestors. This means that, once an object
//! begins mutating its state, it cannot be mutated by outside actors without giving them explicit
//! permission.
//!
//! > <u>An Aside:</u>
//! > These semantics are quite similar to the semantics a functional programmer would have to deal
//! > with when implementing a game in a pure language. Game engines in functional programs take the
//! > form of map functions which map a given object and its inputs to that object's state on the
//! > next frame. The constraints the programmer has to deal with are similar. Functions cannot
//! > mutate a given object unless their ancestor takes that mutation result into account (i.e.
//! > permits the function to perform the mutation). Thus, while functional programmers can
//! > technically contain as many back-references as they wish, these back-references will only
//! > contain immutable snapshots of the previous frame's state.
//!
//! A primitive way of handling these constraints is the [Entity Component System](ecs) (ECS) pattern.
//! Users define a pipeline of "systems" which mutate every object independently (usually filtered
//! by the entities' intersection of components), with a command queue allowing each entity to
//! request modifications to siblings which may be observed later in the pipeline.
//!
//! This design is sufficient to implement the pattern's object-oriented counterpart (direct dispatch
//! to an object can be represented as a deferred call using the command queue) but fails to produce
//! meaningful encapsulation and struggles to promote natural object hierarchy ("system" writers must
//! explicitly support hierarchical data structures and must write such systems in a drastically
//! different manner than the more trivial resource-oriented global system implementation).
//!
//! Crucible's architecture attempts to solve this problem while providing a greater degree of
//! freedom to authors and maintaining idiomaticness.
//!
//! ## Overview
//!
//! Instead of differentiating between systems and their components, Crucible once again unifies
//! behavior and state into reusable objects. Objects, however, do not describe the full state of a
//! "logical entity". Instead, they describe a single independent component of it, and an arbitrary
//! number of objects can reason about a given logical entity without interfering with one another.
//!
//! To unify the notion of a "logical entity" among multiple objects, Crucible defines the notion of
//! an [Entity] and a [Storage]. Storages behave almost identically to the storages in the "component"
//! part of the ECS pattern and provide an efficient mapping between entities and some arbitrary
//! component type. Unlike an ECS, however, storages are plain objects which can be stored anywhere—
//! including within a component stored in a storage—and there can be multiple storages mapping
//! entities to the same component type, allowing users to define natural hierarchies.
//!
//! Thus, the basic unit in Crucible is the object. Objects are reusable units which provide one
//! aspect of a larger logical entity. They can be naturally nested without any change to their design
//! and can fully encapsulate their state using the same encapsulation methods as used in object-oriented
//! programming. They also obviate the need for resources (global state used to coordinate systems
//! in the ECS pattern).
//!
//! Like in an ECS, objects can take in other sibling objects (so long as they don't store
//! back-references). They can also produce arbitrary events which may be handled by a variety of
//! other consumers. In ECS, this took the form of "tag storages" where users would tag affected
//! entities with a component which would then be processed by various listening systems, which had
//! the useful effect of removing heavyweight dynamic dispatch to implement event polymorphism.
//! Crucible takes events handling even further by defining the notion of an [EventPusher] and
//! providing [various event pushing strategies](event). Users may want to handle the event
//! immediately if doing so does not impact cache performance. Others may want to push it into a
//! multithreaded queue so other threads may handle the events in parallel. Others may still want to
//! use the "tag component" strategy and that is still possible in this architecture. These two
//! sibling-dispatch abilities provide a complete replacement for object-oriented programming's
//! direct dispatch (but is arguably more efficient).
//!
//! Sometimes, it may not possible to prove that two objects are siblings at compile time. This
//! happens most frequently when accessing the components of an injective map (e.g. a `Storage` or a
//! `Vec`) at runtime. To fix this, Crucible provides the notion of an [Accessor], a trait for
//! injective maps where non-equal keys are guaranteed to produce two non-aliasing references.
//! Accessors can then be wrapped by [various borrow checking wrappers](accessor) to generically
//! introduce temporary forms of runtime borrow checking.
//!
//! Event pushers (and other abstract dispatch mechanisms such as a `SceneManager` and the client's
//! `MainLoop`) may not know exactly what the event consumers will require. Crucible fixes this by
//! defining the notion of a [Provider], a form of [Accessor] which maps object *types* to object
//! references. Providers act as a form of glue for the rest of the dispatch tree.
//!
//! As a logistical detail, multithreaded execution scheduling is handled through the [Executor],
//! which schedules lightweight `async` tasks on a given number of threads (typically having one for
//! every CPU core). Crucible also provides a large number of `async`-friendly synchronization
//! primitives. Crucible implements an async-friendly [RwLock] where multiple locks can be acquired
//! *atomically* through an [RwGuard]. Crucible also implements a background task system where
//! certain long-running tasks may be paused if higher priority events require processing.
//!
//! TODO: Include links to the various event pushing, borrow checking, and `async` handling strategies.
//!
//! TODO: Implement background tasks, finish accessors and the new ECS, and tag event pushers.
//!
//! ## Tradeoffs with Object-Oriented Programming
//!
//! There are several tradeoffs between object-oriented programming and Crucible's architectural
//! pattern. One of the most major limitations of Crucible's architecture is that it is largely
//! impossible to construct a direct reference to a component instance without some form of heap
//! indirection. This may, in some designs, make accessing deeply nested objects more expensive than
//! necessary. Unfortunately, this problem is inherent to borrow-checker friendly Rust code as
//! storing multiple forward references also violates Rust's strict object tree structure.
//!
//! On the other hand, this strict object tree structure can introduce many incredibly useful
//! properties to Rust code that do not exist in traditional object-oriented programs. Knowing that
//! an object can only be mutated by an enumerable number of consumers can prevent bugs and make safe
//! multithreading much easier. The increased focus on explicit dependency provision also reduces the
//! number of unnecessary object references, reducing memory usage (although [Crew](crew)—the
//! scripting language being made for Crucible—somewhat fixes this with an explicit input system).
//!
//! [crew]: https://github.com/Radbuglet/crew/
//! [ecs]: https://en.wikipedia.org/wiki/Entity_component_system

pub mod event;
pub mod exec;
pub mod lock;
pub mod provider;
pub mod world;

pub mod prelude {
	pub use super::{event::*, exec::*, lock::*, provider::*, world::*};
}

pub use prelude::*;
