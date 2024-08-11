use crucible_utils::{
    define_index,
    hash::FxHashMap,
    newtypes::IndexVec,
    polyfill::{copy_hygiene, OptionExt},
};

use crate::merge::{folders, ArenaMerger, Foldable, FolderExt as _, MapResult, UniqueArenaMerger};

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
    mangler: NameMangler,
}

#[derive(Default)]
struct File {
    types: FxHashMap<String, naga::Handle<naga::Type>>,
    constants: FxHashMap<String, naga::Handle<naga::Constant>>,
    overrides: FxHashMap<String, naga::Handle<naga::Override>>,
    global_variables: FxHashMap<String, naga::Handle<naga::GlobalVariable>>,
    functions: FxHashMap<String, naga::Handle<naga::Function>>,
}

impl ModuleLinker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn link(
        &mut self,
        module: naga::Module,
        span_offset: u32,
        resolve_stub: impl FnMut(&str) -> Option<FileHandle>,
    ) -> FileHandle {
        let mut file = File::default();

        let map_span = |span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                naga::Span::new(v.start as u32 + span_offset, v.end as u32 + span_offset)
            })
        };

        let mut stubs = StubResolver(resolve_stub);

        // Since everything depends on types, let's import those first.
        let types = UniqueArenaMerger::new(
            &mut self.module.types,
            module.types,
            |types, span, ty| {
                // Attempt to map the type to its non-stubbed version
                if let Some((name, file)) = stubs.resolve_opt(ty.name.as_deref()) {
                    return (MapResult::Dedup(self.files[file].types[name]), None);
                }

                // Otherwise, map it so that it can be integrated into the current arena.
                let mut ty = ty.fold(&folders!(a: types, b: &map_span));

                // If it has a name, mangle it.
                let demangle_to;
                if let Some(name) = &mut ty.name {
                    demangle_to = Some(name.clone());
                    self.mangler.mangle_mut(name);
                } else {
                    demangle_to = None
                }

                (MapResult::Map(map_span(span), ty), demangle_to)
            },
            |_types, _dest_arena, demangle_to, _src_handle, dest_handle| {
                if let Some(demangle_to) = demangle_to {
                    file.types.insert(demangle_to, dest_handle);
                }
            },
        );

        // If the imported module had special types, include them. If these types were already set,
        // the type insertion operation should deduplicated them and nothing should change.
        {
            fn update<T: Eq>(target: &mut Option<T>, value: T) {
                assert!(target.is_none_or(|v| &value == v));
                *target = Some(value);
            }

            if let Some(ty) = module.special_types.ray_desc {
                update(
                    &mut self.module.special_types.ray_desc,
                    types.src_to_dest(ty),
                );
            }

            if let Some(ty) = module.special_types.ray_intersection {
                update(
                    &mut self.module.special_types.ray_intersection,
                    types.src_to_dest(ty),
                );
            }

            for (k, ty) in module.special_types.predeclared_types.into_iter() {
                let ty = types.src_to_dest(ty);

                if let Some(&existing) = self.module.special_types.predeclared_types.get(&k) {
                    assert!(existing == ty);
                } else {
                    self.module.special_types.predeclared_types.insert(k, ty);
                }
            }
        }

        // Now, let's define mappings for all the remaining arena objects without modifying the arena
        // entries yet.

        // All these arenas can be mapped by deduplicating by stub.
        macro_rules! new_stubs {
            ($($name:ident),*$(,)?) => {$(
                let mut $name = ArenaMerger::new(
                    &mut self.module.$name,
                    module.$name,
                    |_span, val| {
                        stubs
                            .resolve_opt(val.name.as_deref())
                            .map(|(name, file)| self.files[file].$name[name])
                    },
                );
            )*};
        }

        new_stubs!(constants, overrides, global_variables, functions);

        // This arena doesn't need to be stubbed at all.
        let mut global_expressions = ArenaMerger::new(
            &mut self.module.global_expressions,
            module.global_expressions,
            |_, _| None,
        );

        // Let's fold the new (un-stubbed) values to properly integrate them into the new arena.
        macro_rules! apply_stubs {
            ($($name:ident),*$(,)?) => {$(
                $name.apply(|$name, handle, span, mut val| {
                    if let Some(name) = &mut val.name {
                        file.$name
                            .insert(name.clone(), $name.src_to_dest(handle));

                        self.mangler.mangle_mut(name);
                    }

                    (
                        map_span(span),
                        val.fold(&folders!(
                            // `.upcast()` normalizes both plain `Folder` objects and references
                            // thereto into proper references to `Folder`s.
                            a: copy_hygiene!($name, constants.upcast()),
                            b: copy_hygiene!($name, overrides.upcast()),
                            c: copy_hygiene!($name, types.upcast()),
                            e: copy_hygiene!($name, global_variables.upcast()),
                            g: copy_hygiene!($name, functions.upcast()),
                            h: copy_hygiene!($name, map_span.upcast()),
                            i: copy_hygiene!($name, global_expressions.upcast()),
                        )),
                    )
                });
            )*};
        }

        apply_stubs!(constants, overrides, global_variables, functions);

        global_expressions.apply(|global_expressions, _handle, span, val| {
            // (expressions do not have names to mangle)

            (
                map_span(span),
                val.fold(&folders!(
                    a: &constants,
                    b: &overrides,
                    c: &types,
                    d: global_expressions,
                    e: &global_variables,
                    f: &|_lv| unreachable!("global expressions should not reference local variables"),
                    g: &functions,
                )),
            )
        });

        // Finally, let's import the entry-points.
        // TODO

        self.files.push(file)
    }

    pub fn gen_stubs(&mut self, file: FileHandle, names: &[&str], out: &mut String) {
        todo!()
    }

    pub fn module(&self) -> &naga::Module {
        &self.module
    }
}

#[derive(Debug, Default)]
struct NameMangler(u64);

impl NameMangler {
    pub fn mangle_mut(&mut self, name: &mut String) {
        use std::fmt::Write as _;
        write!(name, "_MANGLE_{:x}_", self.0).unwrap();
        self.0 += 1;
    }

    pub fn demangle<'a>(&mut self, name: &'a str) -> &'a str {
        &name[..name.rfind("_MANGLE").expect("name was never mangled")]
    }
}

struct StubResolver<F>(F);

impl<F> StubResolver<F>
where
    F: FnMut(&str) -> Option<FileHandle>,
{
    pub fn resolve(&mut self, name: &str) -> Option<FileHandle> {
        self.0(name)
    }

    pub fn resolve_opt<'a>(&mut self, name: Option<&'a str>) -> Option<(&'a str, FileHandle)> {
        name.and_then(|name| Some((name, self.resolve(name)?)))
    }
}
