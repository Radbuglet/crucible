# Scripting API

A folder containing my work on the userland scripting API. The API is largely influenced by Godot's [scene tree](https://docs.godotengine.org/en/stable/classes/class_node.html) system with the added notion of namespaced child containers to implement an interface composition mechanism similar to Unity's [game objects](https://docs.unity3d.com/Manual/GameObjects.html).

Thus far, I've been considering two main scripting environments: TypeScript and Rust-backed Wasm. The TypeScript implementation of the object model is feature complete (being much easier to implement) at the expense of performance. The Rust implementation of the Arbre object model, on the other hand, is much more performant, but many of its key features are blocked by rustc's partial implementation of the [`arbitrary_self_types` feature](https://github.com/rust-lang/rust/issues/44874).

Overall, none of this really matters for the first stage of the project—implementing the core game engine—which skips the woes of object-oriented programming for game scripting in favor of the much less flexible but far more performant ECS pattern.
