# Crucible

A work in progress multiplayer voxel game engine to rival... nothing. Made as part of my 2021-2022 independent study.

## Design

This engine borrows a ton from [Godot](https://godotengine.org/), [Unity](https://unity.com/), [Amethyst](https://amethyst.rs/), and the soon(TM) to release game [Hytale](https://hytale.com/). The eventual vision for the engine is to provide a sandbox where users can publish their games to self-hosted servers, with server and client-side plugins implementing all the game's logic.

There are two major design paradigms in this engine: Entity Component Systems (ECS) for the core engine, and scene graphs for the userland scripting environment. The former will be the focus of my 2021-2022 independent study, although I have already worked a bit on prototyping various scripting APIs in the `scripting` folder.

## License

TBD but certainly something Free.

