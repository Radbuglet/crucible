# Arbre Design Overview

**Note:** A proof of concept of this model is already implemented in the `legacy/` directory. It is feature-complete but lacks the key optimizations laid out in this document.

**TODO:** *Add examples.*

Arbre is a user space solution to Rust's "efficient inheritance" problem. It implements an object model where:

- Users can compose classes from an arbitrary number of components, with each component potentially containing sub-components.
- Users can define cross-component dependencies at compile time, creating efficient self-referential structures without `Pin` or redundant references.
- Users can cast class instances at runtime with minimal performance overhead.
- Static dispatch is performed as frequently as possible with minimal generics.
- **TODO:** *Summarize remaining aspects*

## Provider Object System

### Inheritance through Components

A `Provider` is a trait providing a `RawVTable` which allows users to extract sub-components from the object. This v-table is stored in a `PerfectMap` (a `HashMap` that has been computed ahead of time such that there are no collisions), making dynamic component resolution really efficient. Since components can be normal objects with fixed implementations, a dynamic fetch to a statically defined component is almost as efficient as an entirely static dispatch.

**TODO:** *Review `PerfectMap` data structure (see below)*

`Provider` implementations can be written manually in an entirely safe way using the `SafeProvider` utility trait. This is possible because v-tables provide a strongly typed builder version called `VTableFrag` that is associated to the target struct type. These strongly typed v-tables can be generated safely through the `vtable` macro and can be merged with other v-tables using the `extends` and `merge` methods.

```rust
trait Baz {
	fn do_something(&self);
}

#[derive(Default)]
struct Foo {
	bar: Bar,
}

impl SafeProvider for Foo {
	const TABLE: VTableFrag<Self> = vtable!(Self, {
			self: Self, 
			bar: dyn Baz,
		})
		.extends(sub_vtable!(Self, bar))  // When unspecified, the table field defaults to `SafeProvider::TABLE`.
		.extends(sub_vtable!(Self, bar, Bar::gen_another_vtable(MY_KEY)))  // V-table fragments can be defined procedurally.
		.merge(vtable!(Self, {  // Merge can be used to override fields. Duplicate fields raise a compile time error with "extends".
			self: { MY_KEY }
		}));
}

#[derive(Default)]
struct Bar;

impl SafeProvider for Bar {
	const TABLE: VTableFrag<Self> = vtable!(Self, {
		self: Self,
	});
}

impl Bar {
	pub const fn gen_another_vtable(some_ty: TypedKey<dyn Any>) -> VTableFrag<Self> {
		vtable!(Self, {
			self: { some_ty }
		})
	}
}

impl Baz for Bar {
	fn do_something(&self) {
		println!("Did something.");
	}
}

#[test]
fn test() {
	let foo = Box::new(Foo::default());
	foo.fetch::<dyn Baz>().do_something();
}
```

`SafeProvider` is as powerful as it is verbose. In most cases, `Provider` implementations can be generated using the much simpler but (currently) less powerful `provides` macro:

```rust
#[derive(Default)]
struct Bar;

provides!(Bar {  // This macro supports multiple `Provider` definitions if surrounded in braces.
	self: Self,  // This syntax is identical
	..foo,  // This is how we extend a sub-fragment
});

// is equivalent to =>

impl SafeProvider for Bar {
	const TABLE: VTableFrag<Self> = vtable!(Self, {
		self: Self,
	});
}
```

As a convenience feature, tuples whose fields implement `Provider` will also implement `Provider`.

### Static Casting

**TODO:** *Rework such that it works better with Rust's type system, especially when it comes to casting existing objects without taking ownership.*

`ProviderExt` is a trait implementing all the `Provider` querying methods. This trait is automatically implemented for all objects implementing `Provider`. There is only one required method: `try_fetch_key`. This makes it easy for users to define their own `Provider`-like wrappers (e.g. `Node` uses this to implement component querying on node ancestries). Unlike `Provider`, `ProviderExt` is not object safe.

`ProviderExt` can be used for a second purpose: implementing static cast wrappers. Using the `comp_trait` macro, users can define a `#[repr(transparent)]` wrapper struct whose `ProviderExt` implementation includes checks for specific `TypedKeys` that `unwrap_unchecked` their wrapped `Provider`. These wrapper structs, alongside objects implementing `Provider`, also implement `SafeProviderExt`, an unsafe sub-trait of `ProviderExt` that promises that the list of provided components will never change. Wrapper structs can be created from other `SafeProviderExt` objects, ensuring that wrappers created from concrete `Providers` or other wrappers will perform the minimum amount of checks required to validate the cast.

### Component Dependencies

When fetching a component from a `ProviderExt`, the component is wrapped in a `Comp` object, which safely forwards the `self` parameter. **TODO:** *Strengthen semantics to allow us to implement the dependency features safely.*

`Providers` are indexed by `TypedKeys`, which can be created from existing types or dynamic keys. Types are generally used when accessing a public interface, whereas dynamic keys are almost exclusively used for internal cross-referencing.

In the dynamic case, the component contains a reference to a static table containing a mapping from requested dependencies to their `TypedKeys`. Users can then use the provided `Comp`, which requests a `dyn Provider` as the parent parameter, to fetch these dependencies. This strategy is best used when the types of the dependencies are statically known or if the dynamic dependencies are unlikely to be used/used in the slow path.

**TODO:** *Ensure that dynamic dependency provision skips fetch checks.*

If a component requests many dynamic dependencies however, the cost is better paid upfront during the initial dispatch to the component's methods, motivating the static dependency provision strategy. Components using this strategy are generic over their parent `Provider` and accept `deps` as a const generic instead of a reference to a constant. To define methods, users define a trait that is not generic over the parent type. This trait accepts a `dyn Provider` as the `obj` type, but the user can soundly transmute the `dyn Provider` to the generic `Parent` type. All of this is handled with two macros. `#[static_deps(dep_id: DepType)]` is a macro which applies to the component struct, defines a struct of the dependencies, and makes the overall struct generic over `Parent` and `const DEPS`. `#[static_dep_methods(<vis> RouterTraitName)]` is a macro which applies to an impl block, generates the parent-agnostic trait, and implements it for the given block, hiding away the unsafe transmute logic.

**TODO:** *Add partial-access `TypedKeys` to improve static dependency encapsulation.*

## Zero-Cost Locking

Interior mutability is critical for the `Provider` object model since only immutable references to components can be obtained. Interior mutability in single-threaded scenarios is a mere weakening of the aliasing model to C++ levels of aliasing guarantees and is thus still reasonably efficient. However, in multi-threaded scenarios, interior mutability mechanisms become super heavyweight, requiring OS super-vision. Luckily, as many ECS implementations have demonstrated, users only typically need per-service lock granularity. We can emulate this locking system in Arbre using singleton ZSTs, which gets passed through `Providers`.

Keys are defined using the `new_lock_key` macro, which defines a marker struct with methods to acquire both mutable (`WriteKey<LockTy>`) and immutable (`ReadKey<LockTy>`) instance versions of it. Lock mutability follows Rust's XOR mutability rules. `WriteKeys` can be transformed into `ReadKeys`. Users can dynamically request multiple `WriteKeys` on a single thread (checked using `ThreadId` and atomics), although it isn't very efficient.

All locks are `Send` and `Sync` although their provided features may vary depending on whether the content type implements these traits.

Here is a list of all the supported lock-based interior mutability mechanisms:

- `LockCell`, which can accept a `ReadKey` to copy the value in the cell or a `WriteKey` to modify the value in the cell.
- `LockRefCell`, which can accept a `ReadKey` to get an immutable reference to the contents so long as it implements `Sync` or a `WriteKey`, which gives the `LockRefCell` normal `RefCell` functionality.

To reduce the verbosity of passing `ReadKeys` and `WriteKeys` around the application, we can pack keys inside a `Provider` (e.g. using tuple auto `Provider` generation). If the required keys are wrapped in a static cast wrapper, fetch logic can be entirely elided making locks effectively zero-cost. `Providers` are more than a useful bundle however, as they can provide dynamic down-casting, which may be required in cases where the type of the bundle gets upcasted.

**TODO:** *Does this model properly promote abstraction flexibility as it relates to user-defined locks?*

## Object-DB references

Arbre memory management is almost entirely handled through `Orc`, an Object-DB managed version of `std::sync::Arc`. Unlike `Arc`, the memory held by `Orc` can be freed while `WeakOrcs` remain alive. This allows us to implement lazily collected weak collections without having to worry about leaking memory. Furthermore, since `WeakOrc` requires neither a custom `Clone` or `Drop` implementation, they can be freely `Copy`'d around with no extra overhead.

## Weak Collections

Weak collections are collections where the elements they contain are lazily and passively collected while searching for an insertion place or as the collection gets queried. The alive status of an object is determined by its implementation of the `Weak::is_alive` trait method. Right now, we plan on implementing the following collections:

- `WeakValue`: an `Option` which resolves to and stores `None` if the object isn't alive.
- `WeakMap`: a `HashMap` where dead `Weak` elements magically disappear.
- `WeakSet`: see above
- `WeakVec`: an ECS-like `Vec` where dead elements get removed during iteration or resizing.

## Nodes

`Nodes` are a component for use in the `Provider` object model. Nodes serve three purposes in Arbre: they can provide dependencies based off the node hierarchy, they can be used for logical group iteration (e.g. replicating a branch of game objects), and they can associate a child's lifetime with its parent's.

Since `Nodes` store the object's parent, users can easily query their ancestors. Coupled with a special `ProviderExt` implementation, users can fetch components from their ancestors, implementing a simple and convenient dependency injection system.

`Nodes` contain their descendants in a linked list of `Orc<WithNode<dyn Provider>>`. This enables us to implement a node hierarchy using the same exact object lifetime rules as normal `Orcs`. Coupled with a relative depth variable, users can iterate through a node's descendant list. Because all nodes are `Providers`, users can dynamically cast these children to check if they have a required component, making the node hierarchy perfect for notifying large logically grouped portions of a game world.

Since `Nodes` are present in almost all objects in the game world, `WeakMaps` can use them to implement `Storages`. `Storages` implement an SOA annotation mechanism where an arbitrary object with a `Node` component is mapped to a child object which dies when either the `Storage` or its parent die. The `Storage` is a simple `WeakMap` mapping `WeakOrcs` to the parent to `WeakOrcs` to the children that implements custom `Drop` logic to deparent the contained objects. In cases where the map's value does not require a predictable `Drop` call, `WeakMaps` can be used.

---

# `PerfectMap` Performance

`PerfectMap` can only realistically solve for 16 component v-tables and doing so requires a high `const_eval` step limit. Ideally, we'd find a better algorithm that results in both a speedy lookup and a scalable building algorithm. We'd also like to keep the load factor relatively low although this objective is less critical so long as the maps don't get too big (we should be able to handle ~500 in a 2mb static section).

## Work required for 100% load factor

Currently, `PerfectMap` works by repeatedly randomizing the hashes with an ever-increasing seed and checking if the hash mapping is perfect for the given key set.

The probability of randomly encountering such a mapping is $\frac{n!}{x^x}$ assuming each bucket has an equal probability of being selected.

The probability of being successful after $n$ attempts is $(1-\frac{x!}{x^x})^n$.

The average number of attempts required to perfectly hash $x$ buckets with $a$ success rate is $n=\log_{(1-\frac{x!}{x^x})}(a)$.

This graph is scary and bad.

## Load factor for limited work

The probability of randomly encountering a perfect mapping given $n$ keys and $m$ buckets is $\frac{m!}{m^{n}(m-n)!}$ assuming each bucket has an equal probability of being selected.

**TODO**

## Performance cost of double-layered FHS hashing

**TODO**

# `Comp` Semantics

**TODO:** *TBD*
