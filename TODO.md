# To-Do

## Core

- [x] **Foundations:** Implement `Accessors` and document them.
- [ ] **General:** Integrate `Accessors` into `VoxelData`, `ViewportManager`, `Storages`, and `Providers`.
- [ ] **Foundations:** Make `RwGuards` into `Providers` to simplify external guard decomposition.
- [ ] **Foundations:** Implement `DynProvider`.
- [ ] **Foundations:** Finish implementing archetypal ECS
- [ ] **Foundations:** Implement ECS-style tagging `EventTarget` (single and multi layered), multithreaded `VecDequeue`, fixed-size `Array`
- [ ] **Foundations:** Automate dependency currying with forwarders (or an `impl_curried!` macro).
- [ ] **Foundations:** Implement `DynEventBus` with stage futures.

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
