# To-Do

## Client Engine

- [x] Implement a uniform manager \
      While most of what would be done with OpenGl style uniforms are implemented using bespoke usage-specific infrastructure, there's still a hand-full of small uniforms that need to be stored somewhere. They might also be important for providing a fallback when hardware doesn't support push constants.
- [ ] Implement frame object queue (mostly used for minimizing map block time).
- [ ] Clean up `main`:
  - [ ] Make event routing more declarative
  - [ ] Handle depth buffer creation somewhere else
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

## General Things

- [ ] Make the core engine panic-proof:
  - [ ] Don't make assumptions about the extremes (e.g. chunk pos addition)
  - [ ] Review GPU handling (there's a lot of things that can implicitly break here)
  - [ ] Entity lifetime handling (we run `get` a lot even though users could delete the entities at any time) 
  - [ ] Register crash handlers
