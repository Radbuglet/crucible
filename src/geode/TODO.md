# To-Do

- [ ] Simple API changes:
  - [ ] Replace `Owned: Deref` with an explicit call to `.weak_ref()`. Safe methods that don't
        accidentally produce weak references can be forwarded.
  - [ ] Implement `Node`, a way to attach a dependency hierarchy to an `Entity`.
  - [ ] Implement `CopyVec`, `CopyHashMap`, and `CopyHashSet`.
  - [ ] Expose more of the `Obj` lifecycle methods to `Entity`.
- [ ] Signals:
  - [ ] `event_trait` multiplexing, packing, conversions, and forwarding.
  - [ ] `InstantSignal` and `DeferredSignal` object (once `Storage` becomes available)
- [ ] GC prep:
  - [x] Clean up `Sessions` implementation (i.e. clarify invariants)
  - [ ] Put non-reentrant methods in their own layer to avoid recursive calls.
  - [ ] Unify `Obj` and `RawObj` with `ReflectType`. We can force the compiler to inline the underlying
        generation code to keep codegen equivalent.
  - [ ] Improve bad `ExtendedGen` typing (it's currently a massive footgun)
  - [ ] Fix the `Slot` release routine (the free slot can only be added to the list of the thread
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
  - [ ] Document the API
