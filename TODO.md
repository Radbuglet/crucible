# To-Do

## Foundations

- [ ] Implement ECS-style queries
- [ ] Implement storage wrappers \
      There's some cases where users might want to sacrifice some fetch performance to allow multiple arbitrary borrow from a storage. Other times, services may want to return a fetch-only version of their storage.
- [ ] Improve executor flexibility \
      *e.g.* make it possible to pause and resume tasks, add a system for batch processing with various criticality levels
- [ ] Improve dependency provision \
      `Providers` are mostly used to simplify context passing to lifetime-limited services (*e.g.* passing engine state to the run loop, storing arbitrary modules in the module manager), but they could play an important role in reducing the verbosity of service singleton signatures. Here are the specific improvements needed:
  - [ ] Typed providers
  - [ ] Fetching from RW lock guards
  - [ ] Mutable borrowing and splitting
- [ ] Update module documentation \
  It should mention the idea of `Storage` hierarchies and opt-in back-references, which have become massively important for services like the voxel renderer. It should generally give the impression that storages are more-so a specialized version of `TypedKeys` that works better with Rust's aliasing rules than the impression that we're trying to use a strictly systems-engineered architecture in this project. We should also mention where to use providers over storages since they implement the same mechanism but have two very different use-cases.

## Client Engine

- [x] Implement a uniform manager \
      While most of what would be done with OpenGl style uniforms are implemented using bespoke usage-specific infrastructure, there's still a hand-full of small uniforms that need to be stored somewhere. They might also be important for providing a fallback when hardware doesn't support push constants.
- [ ] Implement frame object queue (mostly used for minimizing map block time).
- [ ] Implement a directory manager \
      We cannot embed all assets into the binary and not all target platforms support real filesystems.
- [ ] Implement a voxel data container:
  - [ ] Chunk linking
  - [ ] Tile entities
  - [ ] Lyptic-style ray-casts
  - [ ] Lyptic-style rigid-bodies
- [ ] Implement a voxel renderer:
  - [x] Efficient voxel face representation
  - [ ] Voxel mesh packing
  - [ ] Multithreaded re-meshing
  - [ ] Chunk hull culling with internal face visibility graphs
  - [ ] Custom block meshes
