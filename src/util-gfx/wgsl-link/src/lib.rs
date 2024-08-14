//! A namespace-safe linker for naga-parseable modules.

// Here's our linking algorithm:
//
// - For each file of the dependency graph in toposorted order...
// - Find the dependencies of the file and generate stub source code for each of them with the
//   expected name using the dependencies' module information. For structures, the non-direct imports
//   must be mangled.
// - Parse each file and generate a module for it.
// - Link at the module level.
// - Mangle the colliding names.
// - Emit a giant file for the entire linked module.
//

pub mod driver;
pub mod module;
