# Geode To-Do

- [ ] Implement single-threaded `Obj` proof-of-concept:
  - [ ] Implement borrowing models:
    - [ ] Basic `.borrow_ref` and `.borrow_mut`
    - [ ] Basic `.fetch_many`
    - [ ] Macro-based `.fetch_many`
    - [ ] Closure borrowing
    - [ ] `.borrow_raw`, `.to_raw`, `.downgrade`, `.upgrade`, `.loan`
  - [ ] Integrate with `EventTarget`:
    - [ ] Write `add_event_target` helper
    - [ ] Write `fire_event` helper
  - [ ] Integrate with context trees
  - [ ] Add `DebugLabel`
  - [ ] Add dynamic, static, and semi-static component keys
- [ ] Implement multithreaded `Obj`:
  - [ ] Integrate with `RwLockManager`
  - [ ] Remove limits on `RwLockManager`
  - [ ] Add single-threaded mutable borrow wrapper for `Obj`.
  - [ ] Add support for `async` borrowing variants
- [ ] Improve `Accessors`:
  - [ ] Less verbose signatures
  - [ ] Better temporaries
  - [ ] Map accessor
  - [ ] `RefCell` accessor
  - [ ] Better splitting
  - [ ] Iteration
- [ ] Improve ECS:
  - [ ] Finish archetypal memoization and cleanup
  - [ ] Implement proper flushing
  - [ ] Improve dirty checks API
  - [ ] Integrate `MapStorage` with `WorldAccessor`
  - [ ] Write `ArchStorage`
  - [ ] Write `Query`
  - [ ] Integrate with `Accessors`
- [ ] Document:
  - [ ] README
  - [ ] Module documentation
  - [ ] Examples
  - [ ] Unit tests
  - [ ] Stabilize nightly features

## Example

```rust
use geode::prelude::*;
use macroquad::prelude::*;

struct EntityHandleAiTick;
struct UpdateEvent;
struct RenderEvent;

#[macroquad::main("Lyptic")]
async fn main() {
  // Initialize engine
  let mut root = Obj::new();
  root.add(DebugLabel::new("engine root"));
  root.add(World::new());
  root.add(SceneManager::new());

  {
    let mut sm = root.borrow_mut::<SceneManager>();
    sm.set_next_scene(make_play_scene());
  }

  // Run main loop
  while !is_quit_requested() {
    // Swap scenes
    let mut sm = root.borrow_mut::<SceneManager>();
    sm.swap_scene();
    
    // Process current scene
    let sm = RwMut::downgrade(sm);
    let current_scene = sm.current_scene();
    current_scene.fire(UpdateEvent);
    current_scene.try_fire(RenderEvent);
    
    // Finish frame
    next_frame().await;
  }
}

fn make_play_scene() -> Obj {
  let entity_kinds_key = new_dynamic_key::<Vec<_>>();

  let mut scene = Obj::new();
  scene.add(DebugLabel::new("play scene"));
  scene.add(GameMap::new());
  scene.add_as(entity_kinds_key, vec![
    make_zombie_handler(),
  ]);
  scene.add_event_handler(|_: UpdateEvent, cx: &ObjCx, map: RwMut<GameMap>| {
    // Update entities
    let mut kinds = cx.borrow_ref(entity_kinds_key);
    for handler in kinds.iter() {
      handler.fire(EntityHandleAiTick);
    }
    
    // ...
  });
}

fn make_zombie_handler() -> Obj {
  struct ZombieState {
    // ...
  }

  let mut bundle = Obj::new();
  bundle.add(ArchStorage::<ZombieState>::new());
  bundle.add_event_handler(|_: EntityHandleAiTick, world: &World| {
    // ...
  });
}
```
