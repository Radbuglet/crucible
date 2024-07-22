use crucible_utils::{define_index, hash::FxHashMap, newtypes::IndexVec};

use crate::merge::{folders, Foldable, MapResult, UniqueArenaMerger};

define_index! {
    pub struct FileHandle: u32;
}

#[derive(Default)]
pub struct ModuleLinker {
    // The entire link context is contained within this module. We apply tree-shaking to produce
    // the final module to allow the linker to be effectively reused between compilation sessions.
    module: naga::Module,

    // Maps individual files and their exports into handles into the giant module we're constructing.
    files: IndexVec<FileHandle, File>,

    // ID generator for de-mangling.
    generator: u64,
}

#[derive(Default)]
struct File {
    functions: FxHashMap<String, naga::Handle<naga::Function>>,
    structures: FxHashMap<String, naga::Handle<naga::Type>>,
}

impl ModuleLinker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn link(
        &mut self,
        module: naga::Module,
        span_offset: u32,
        mut resolve_extern: impl FnMut(&str) -> Option<FileHandle>,
    ) -> FileHandle {
        let file = File::default();

        let map_span = |span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                naga::Span::new(v.start as u32 + span_offset, v.end as u32 + span_offset)
            })
        };

        let mut mangle = |mut name: String| -> String {
            use std::fmt::Write as _;
            write!(&mut name, "_mangle_{:x}", self.generator).unwrap();
            self.generator += 1;
            name
        };

        // Since everything depends on types, let's import those first.
        let ty_map =
            UniqueArenaMerger::new(&mut self.module.types, module.types, |ty_map, span, ty| {
                if let Some(file) = ty.name.as_deref().and_then(&mut resolve_extern) {
                    MapResult::Dedup(self.files[file].structures[ty.name.as_deref().unwrap()])
                } else {
                    let mut ty = ty.fold(&folders!(a: ty_map, b: &map_span));
                    ty.name = ty.name.map(&mut mangle);
                    MapResult::Map(map_span(span), ty)
                }
            });

        // TODO: Map everything else!

        self.files.push(file)
    }

    pub fn module(&self) {}
}
