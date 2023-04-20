# Crucible Engine Architecture

Radbuglet • Last Updated: 4/20/23

## Layer 0: Utilities

- `bort`: implements the object model on which the rest of the game is built
- `crucible-util`: non-`Bort` utilities for the Rust programming language in general
- `typed-glam`: traits for the various `glam` vector types and a mechanism for generating vector new-types.
- `typed-wgpu`: strongly typed wrappers around `wgpu` objects

## Layer 1: Foundation

- `crucible-foundation`: generic voxel data structures that are used to build both the `core` plugin and other game plugins with more specific needs; essentially, these are the less opinionated versions of the concepts exposed in the `core` plugin.
  - `math`: engine-specific math objects (mostly axis aligned objects and custom coordinate systems)
    - `kinematic`: defines additional kinematic math for simple physics models
  - `material`: defines a material database, which is used by both the voxel world and the item system
  - `voxel`: voxel world data-structures and querying mechanisms 
    - `data`: stores voxel data and provides utilities for efficiently navigating it
    - `mesh`: defines the notion of a voxel mesh, which is used by the client for rendering and the server for collision detection
    - `collision`: defines low-level collision detection utilities against the voxel world
  - `actor`: defines actor tracking and generic utilities
    - `manager`: manages actors and their archetypes
    - `spatial`: defines mechanisms for tracking and querying entities spatially
    - `collision`: defines entity-specific collision detection on top of some of the world's mechanisms
  - `plugin`: defines the plugin system
    - `core`: defines the plugin manager
    - `task`: defines the task system

## Layer 2: Core Plugins

- `crucible-common`:
  - `core`: A single plugin defining...
    - A global material registry
    - A global world registry
    - A global entity system
    - A global scheduler system

## Layer 1, Client: IO Foundation

==TODO==

## Layer 2, Client: Core Plugins

==TODO==

## Layer 3: Game Plugins

==TODO==
