# Crucible

A work in progress multiplayer voxel game engine to rival... nothing. Made as part of my 2021-2022 independent study.

## Design

This engine borrows a ton from [Godot](https://godotengine.org/), [Unity](https://unity.com/), [Amethyst](https://amethyst.rs/), and the soon(TM) to release game [Hytale](https://hytale.com/). The eventual vision for the engine is to provide a sandbox where users can publish their games to self-hosted servers, with server and client-side plugins implementing all the game's logic.

There are two major design paradigms in this engine: Godot-like servers (core engine) and Unity-like component trees (plugins).

Servers are singletons which are ticked automatically through a multithreaded `Executor`, which reorganizes and parallelizes tasks in accordance to a task dependency graph. Each singleton defines a series of read-write locks whose access is mediated by the execution engine. Singletons can communicate through immediate access (holding a write-lock) or by queuing actions (pushing to a special `Dequeue` designed for multithreaded use). All tasks have access to the lock-free `EntityManager` server, which unifies server-specific object identities with their cross-server `EntityId` representation.

While the server architecture is great for finely tuning performance, it does not provide the required flexibility for game scripting. Thus, user-land code (including the engine's user interfaces) is written using Arbre, a Rust library which implements an object paradigm similar to Unity's `GameObjects`. Arbre is currently being developed in the [optimized-vtable](https://github.com/Radbuglet/crucible/tree/optimized-vtable) branch.

## License

TBD but certainly something Free.

