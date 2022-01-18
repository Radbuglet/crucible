# To-Do

## Core

- [ ] **Foundations:** Implement `Accessors`
- [ ] **General:** Integrate `Accessors` into `VoxelData`, `ViewportManager`, `Storages`, and `Providers`
- [ ] **Foundations:** Make `RwGuards` into `Providers` to simplify external guard decomposition
- [ ] **Foundations:** Finish implementing archetypal ECS
- [ ] **Foundations:** Implement ECS-style tagging `EventTarget`, multithreaded `VecDequeue`, fixed size `Array`, and event target reflection/multiplexing for plugin event busses.
- [ ] **Foundations:** Somehow make contextual binding less of a chore (`VoxelWorld`, for instance, needs context binding to ensure that outside users will appropriately signal mesh updates to the renderer)

## Basic

- [x] **Client:** Clean up main loop:
  - [x] Implement `derive(Provider)` for `Engine` object.
  - [x] Standardize `ViewportManager` handling into `Engine.handle_swapchain_rebuild`; move depth buffer management into its own service.
- [x] **Client:** Improve mesh buffer storage.
- [x] **Client:** Implement a custom (non-UB) std140 system.
- [x] **Client:** Reduce thread blocking from buffer memory mapping.
- [ ] **Client:** Make initialization more robust (dynamic swap-chains, better feature detection)
- [ ] **Common:** Formalize coordinate spaces
- [ ] **Client:** Asset loader
- [ ] **Client:** Textured voxels
- [ ] **Client:** Flood fill voxel lighting
- [ ] **Client:** Entity mesh renderer (integrate with lighting)
- [ ] **Client:** Non-cubic voxel renderer
- [ ] **Foundations:** Finish implementing multi-threading mechanisms; integrate
- [ ] **General:** Improve panic safety

## Scriptify

- [ ] **Server:** Implement a headless instance
- [ ] **Client & Server:** Implement a socket
- [ ] **Client & Server:** Plugin API
- [ ] **Client:** Plugin sandbox and loading
- [ ] **Nautilus:** Implement foundation API
- [ ] **Nautilus:** Implement engine root
