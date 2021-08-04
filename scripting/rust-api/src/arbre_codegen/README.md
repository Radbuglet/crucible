# Arbre Codegen

Tracks Arbre codegen quality for all four methods in the Arbre dispatch matrix.

## Results

Last ran: `July 14th, 2021`

`rustc` version:

```
rustc 1.55.0-nightly (3e1c75c6e 2021-07-13)
binary: rustc
commit-hash: 3e1c75c6e25a4db968066bd2ef2dabc7c504d7ca
commit-date: 2021-07-13
host: x86_64-pc-windows-msvc
release: 1.55.0-nightly
LLVM version: 12.0.1
```

Performance:

- `fetch_static_static`: optimal
- `fetch_static_dynamic`: optimal
- `fetch_dynamic_static`: dubious (large fetch stub, optimal dispatch)
- `fetch_dynamic_dynamic`: dubious (large fetch stub, reasonable dispatch)

See `src/arbre_codegen.s` for more details.
