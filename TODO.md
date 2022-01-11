# To-Do

## Basic

- [ ] **Clean up:** Remove unnecessary nightly features; upgrade underlying toolchain before we foot-gun.
- [ ] **Foundations:** Implement `Accessors`
- [ ] **General:** Integrate `Accessors` into `VoxelData`, `ViewportManager`, `Storages`, and `Providers`
- [ ] **Foundations:** Make `RwGuards` into `Providers` to simplify external guard decomposition
- [ ] **Foundations:** Finish implementing archetypal ECS
- [ ] **Foundations:** Implement `EventPushers`: ECS-style tagging, multithreaded `VecDequeue` pusher, `SmallVec` pusher, `Array` pusher, `EventBus`, and `Multiplexer`
- [ ] **Client:** Clean up main loop
- [ ] **Client:** Implement a custom (non-UB) std140 system
- [ ] **Client:** Make a more robust frame resource system
- [ ] **Client:** Make initialization more robust (dynamic swap-chains, better feature detection)
- [ ] **Common:** Formalize coordinate spaces
- [ ] **Client:** Profile rendering, improve mesh buffer storage
- [ ] **Client:** Asset loader
- [ ] **Client:** Textured voxels
- [ ] **Client:** Flood fill voxel lighting
- [ ] **Client:** Entity mesh renderer (integrate with lighting)
- [ ] **Client:** Non-cubic voxel renderer
- [ ] **Foundations:** Finish implementing multi-threading mechanisms; integrate

## Scriptify

- [ ] **Server:** Implement a headless instance
- [ ] **Client & Server:** Implement a socket
- [ ] **Client & Server:** Plugin API
- [ ] **Client:** Plugin sandbox and loading
- [ ] **Nautilus:** Implement foundation API
- [ ] **Nautilus:** Implement engine root
