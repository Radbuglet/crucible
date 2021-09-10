# Crucible

A work in progress multiplayer voxel game engine to rival... nothing. Made as part of my 2021-2022 independent study.

## Design

This engine borrows a ton from [Godot](https://godotengine.org/), [Unity](https://unity.com/),
[Amethyst](https://amethyst.rs/), and the soon(TM) to release game [Hytale](https://hytale.com/). The eventual vision
for the engine is to provide a sandbox where users can publish their games to self-hosted servers,  with server and
client-side plugins implementing all the game's logic.

There are two major design paradigms in this engine: Entity Component Systems (ECS) for the core engine, and scene
graphs for the userland scripting environment. The former component is the focus of my 2021-2022 independent study,
and an architectural overview can be found in the module-level documentation of `core::foundation`. I have already
prototyped a few scripting frameworks in the `scripting/` directory, but I'm likely going to end up using a custom
scripting language called [Crew](https://github.com/Radbuglet/crew).

## License

TBD but certainly something Free.

