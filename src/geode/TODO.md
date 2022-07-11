# To-Do

- [ ] Simple API changes:
  - [ ] Implement `Node`, a way to attach a dependency hierarchy to an `Entity`.
  - [ ] Implement `CopyVec`, `CopyHashMap`, and `CopyHashSet`.
  - [ ] Expose more of the `Obj` lifecycle methods to `Entity`.
  - [x] Expose fallible getters to `Obj`.
- [ ] Signals:
  - [ ] `event_trait` multiplexing, packing, conversions, and forwarding.
  - [ ] `InstantSignal` and `DeferredSignal` object (once `Storage` becomes available)
- [x] GC prep:
  - [x] Clean up `Sessions` implementation (i.e. clarify invariants)
  - [x] Put non-reentrant methods in their own layer to avoid recursive calls.
  - [x] Unify `Obj` and `RawObj` implementations.
  - [x] Fix the `Slot` release routine (the free slot can only be added to the list of the thread
        that deleted it)
- [ ] GC implementation:
  - [ ] Implement `Heap` compaction.
  - [ ] Implement basic GC finalization and compaction routine.
  - [ ] Extend with post-finalization hooks that can be associated with an `Obj`.
  - [ ] Implement the multi-heap system.
- [ ] Entity implementation:
  - [ ] `PerfectHashMap` implementation
  - [ ] Archetype RC
  - [ ] Archetype heap packing
  - [ ] `Storage` and `CopyStorage` types
- [ ] Future:
  - [ ] Benchmark final performance
  - [ ] Use guard pages to make allocation API branchless (probably not necessary; most other ECS'
        have pretty expensive entity creation routines)
  - [ ] Implement multithreaded compaction
  - [ ] Allow multithreaded finalization
  - [ ] Write internals document
  - [ ] Add RW session locks to emulate ECS-style read-only component acquisition.
  - [ ] Document the API
