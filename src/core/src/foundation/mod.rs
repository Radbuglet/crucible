//! The module implementing Crucible's core engine infrastructure.
//!
//! ## Background
//!
//! Crucible's architecture is largely impacted by one central constraint: the borrow checker. In
//! order for the borrow checker to provide useful results, users are only capable of borrowing an
//! object mutably once. In Rust, application state forms a strict tree where objects needing
//! mutation can never contain back-references to their ancestors (without resorting to heavyweight
//! runtime-tracked shared mutability objects such as `Arc<Mutex<...>>`). Because of these limitations,
//! this means that, once an object begins mutating its state, it cannot be mutated by outside actors
//! without giving them explicit permission to do so.
//!
//! > <u>An Aside:</u>
//! > These semantics are quite similar to the semantics a functional programmer would have to deal
//! > with when implementing a game in a pure language. Game engines in functional programs take the
//! > form of map functions which map a given object and its inputs to that object's state on the
//! > next frame. The constraints of this technique are similar: functions cannot mutate a given
//! > object unless their ancestor takes that mutation result into account (i.e. permits the function
//! > to perform the mutation). Thus, while functional programmers can technically contain as many
//! > back-references as they wish, these back-references will only contain immutable snapshots of
//! > the previous frame's state.
//!
//! A common way of handling these constraints is the [Entity Component System](ecs) (ECS) pattern.
//! Users define a pipeline of "systems" where each system mutates every object independently (usually
//! filtered by the entities' intersection of components), with a command queue allowing each entity
//! to request modifications to siblings which may be observed later in the pipeline. To store
//! references to other components, users can store an entity ID for use in the command queue, enabling
//! the creation of shared mutable "weak references" without breaking the underlying language's
//! ownership rules.
//!
//! This design is sufficient to achieve feature parity with the pattern's object-oriented counterpart
//! (direct dispatch to an object can be represented as a deferred call using the command queue) but
//! fails to produce meaningful encapsulation and struggles to promote natural object hierarchy ("system"
//! writers must explicitly support hierarchical data structures, with such a design being entirely
//! different and far more complex than the more limited flat hierarchy).
//!
//! Crucible's architecture attempts to solve this problem while providing a greater degree of
//! freedom to authors and maintaining idiomaticness.
//!
//! ## Overview
//!
//! Instead of differentiating between systems and their components, Crucible once again unifies
//! behavior and state into reusable *objects*. Objects, however, do not describe the full state of a
//! "logical entity". Instead, they describe a single independent component of it, and an arbitrary
//! number of objects can reason about a given logical entity without interfering with one another
//! (this is true at both the abstraction and concurrent data access levels).
//!
//! To unify the notion of a "logical entity" among multiple objects, Crucible defines the notion of
//! an [Entity] and a [Storage]. Storages behave almost identically to the storages in the "component"
//! part of the ECS pattern and provide an efficient mapping between entities and some arbitrary
//! component type. Unlike an ECS, however, storages are plain objects which can be stored anywhere—
//! including within a component stored in a storage—and there can be multiple distinct storages
//! mapping entities to the same component type, allowing users to define natural hierarchies.
//!
//! Objects in Crucible are almost identical to class instances in object-oriented programming
//! languages but deviate in the ways in which they store references to one another. Objects can take
//! in references to other sibling objects so long as they are not one of the object's direct ancestors.
//! However, unlike references in non-single-ownership languages, these references are ephemeral (they
//! cannot be stored within the object instance) and serve more as contextual dependencies than as
//! links to other objects in a graph. To handle actual cross-references, one must use an [Entity] ID.
//!
//! Sometimes, outside users may need to access a child object mediating a subsystem of its parent but
//! may not be trusted with properly providing all the correct context to its methods. Users are
//! encouraged to ["curry"](currying) this context using context objects.
//!
//! This complicates things when trying to manipulate components which are not direct descendants of
//! the current object. If the target object is the descendant of a sibling being passed directly
//! to the calling object, the caller can simply resolve the target object from that sibling. However,
//! in cases where the target object is deeply nested, used concurrently by a different thread, or
//! exhibits polymorphic behavior, it may be undesirable to halt the hot inner loop of the object to
//! fetch data far away from the active cache lines. Thus, calls to sibling entities may sometimes be
//! deferred to an external actor to handle later by accepting objects implementing the [EventTarget]
//! trait.
//!
//! To compliment this standard trait, this module provides [various standard event handling strategies](event).
//! Users can handle the event immediately using regular `FnMut` closures if doing so does not impact
//! cache performance or contend with active locks. Alternatively, users could push to a dequeue by
//! passing in a regular [VecDequeue](std::collections::VecDeque) as their event target. Others may
//! wish to use the existing archetypal querying machinery used by [Storage] queries to place
//! ephemeral "trigger" tags on affected entities using the [EventTargetArchetypeTag] event target.
//! Others, still, may want to only iterate over affected branches of the logical entity tree using
//! the [EventTargetPathQueue] event target.
//!
//! The targets of an signal dispatcher may also implement the `EventTarget` trait. Users can use
//! the [dyn_event_target] macro to derive reflection traits on top of the `EventTarget` implementations
//! for use with plugin-style event busses.
//!
//! Sometimes, it may not possible to prove that two objects are siblings at compile time. This
//! happens most frequently when accessing the components of an injective map (e.g. a `Storage` or a
//! `Vec`) at runtime. To fix this, Crucible provides the notion of an [Accessor], a trait for
//! injective maps where non-equal keys are guaranteed to produce two non-aliasing references.
//! Accessors can then be wrapped by [various borrow checking wrappers](accessor) to generically
//! introduce temporary forms of runtime borrow checking. Because most wrappers apply to both immutable
//! and semi-mutable accessors in the same manner, accessors may also reduce the boilerplate needed
//! to provide access methods for all combinations of mutability.
//!
//! Event targets (and other abstract dispatch mechanisms such as a `SceneManager` and the client's
//! `MainLoop`) may not know exactly what their consumers will require. Crucible fixes this by defining
//! the notion of a [Provider], a form of [Accessor] which maps object *types* to object references.
//!
//! ## Tradeoffs with Object-Oriented Programming
//!
//! There are several tradeoffs between object-oriented programming and Crucible's architectural
//! pattern. One of the most major limitations of Crucible's architecture is that it is largely
//! impossible to construct a direct reference to a component instance without some form of heap
//! indirection. This may, in some designs, make accessing deeply nested objects more expensive than
//! otherwise necessary. Unfortunately, this problem is inherent to borrow-checker friendly Rust code
//! as storing multiple forward references violates Rust's strict object tree structure.
//!
//! On the other hand, this strict object tree structure can introduce many incredibly useful
//! properties to Rust code that do not exist in traditional object-oriented programs. Knowing that
//! an object can only be mutated by an enumerable number of consumers can prevent bugs and make safe
//! multithreading much easier. The increased focus on explicit dependency provision also reduces the
//! number of unnecessary object references, reducing memory usage (although [Crew](crew)—the
//! scripting language being made for Crucible—somewhat fixes this with an explicit input system).
//!
//! [crew]: https://github.com/Radbuglet/crew/
//! [currying]: https://en.wikipedia.org/wiki/Currying
//! [ecs]: https://en.wikipedia.org/wiki/Entity_component_system

pub mod accessor;
pub mod event;
pub mod lock;
pub mod provider;
pub mod world;

pub mod prelude {
	pub use super::{accessor::*, event::*, lock::*, provider::*, world::*};
}

pub use prelude::*;
