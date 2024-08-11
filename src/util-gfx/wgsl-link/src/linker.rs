use crucible_utils::{define_index, hash::FxHashMap, newtypes::IndexVec, polyfill::OptionExt};

use crate::merge::{folders, ArenaMerger, Foldable, MapResult, UniqueArenaMerger};

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
        let file = File::default();

        let map_span = |span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                naga::Span::new(v.start as u32 + span_offset, v.end as u32 + span_offset)
            })
        };

        let mut stubs = StubResolver(resolve_stub);

        // Since everything depends on types, let's import those first.
        let types =
            UniqueArenaMerger::new(&mut self.module.types, module.types, |ty_map, span, ty| {
                // Attempt to map the type to its non-stubbed version
                if let Some((name, file)) = stubs.resolve_opt(ty.name.as_deref()) {
                    return MapResult::Dedup(self.files[file].types[name]);
                }

                // Otherwise, map it so that it can be integrated into the current arena.
                let mut ty = ty.fold(&folders!(a: ty_map, b: &map_span));

                // If it has a name, mangle it and mark down the mangling from real name to handle.
                if let Some(name) = &mut ty.name {
                    // TODO: Mangle name and update mangle map
                }

                MapResult::Map(map_span(span), ty)
            });

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
        // TODO: Mangle name and update mangle map
        constants.apply(|_, span, val| {
            (
                map_span(span),
                val.fold(&folders!(a: &global_expressions, b: &types)),
            )
        });

        overrides.apply(|_, span, val| {
            (
                map_span(span),
                val.fold(&folders!(a: &global_expressions, b: &types)),
            )
        });

        global_variables.apply(|_, span, val| {
            (
                map_span(span),
                val.fold(&folders!(a: &global_expressions, b: &types)),
            )
        });

        functions.apply(|functions, span, val| {
            (
                map_span(span),
                val.fold(&folders!(
                    a: &constants,
                    b: &overrides,
                    c: &types,
                    d: &global_variables,
                    e: functions,
                    f: &map_span,
                )),
            )
        });

        global_expressions.apply(|global_expressions, span, val| {
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
                ))
            )
        });

        // Finally, let's import the entry-points.
        // TODO

        self.files.push(file)
    }

    pub fn gen_stubs(&mut self, file: FileHandle, names: &[&str], out: &mut String) {
        todo!()
    }
}

#[derive(Debug, Default)]
struct NameMangler(u64);

impl NameMangler {
    pub fn mangle_mut(&mut self, name: &mut String) {
        use std::fmt::Write as _;
        write!(name, "_MANGLE_{:x}", self.0).unwrap();
        self.0 += 1;
    }

    pub fn mangle_owned(&mut self, mut name: String) -> String {
        self.mangle_mut(&mut name);
        name
    }

    pub fn de_mangle<'a>(&mut self, name: &'a str) -> &'a str {
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
