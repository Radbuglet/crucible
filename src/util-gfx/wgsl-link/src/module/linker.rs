use crucible_utils::{
    define_index,
    hash::{FxHashMap, FxStrMap},
    newtypes::{IndexVec, LargeIndex as _},
    polyfill::{copy_hygiene, OptionExt},
};
use naga::Span;

use crate::mangle::{mangle_mut, replace_mangles, try_demangle, MangleIndex};

use super::{
    fold::{folders, Foldable as _, FolderExt as _},
    merge::{ArenaMerger, MapResult, RawNagaHandle, UniqueArenaMerger},
    shake::{ArenaShakeSession, ArenaShaker, UniqueArenaShaker},
};

// === ModuleLinker === //

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

    // Map from mangle IDs to handles.
    mangle_idx_to_handle: IndexVec<MangleIndex, RawNagaHandle>,
}

impl ModuleLinker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn link(
        &mut self,
        module: naga::Module,
        stubs: &ImportStubs,
        span_offset: u32,
    ) -> FileHandle {
        let mut file = File::default();

        let map_span = |span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                naga::Span::new(v.start as u32 + span_offset, v.end as u32 + span_offset)
            })
        };

        // Since everything depends on types, let's import those first.
        macro_rules! dedup_name {
            ($name:expr, $expected_kind:expr) => {
                'a: {
                    let name = $name;

                    if let Some((_, mangle_idx)) = try_demangle(name) {
                        break 'a Some(self.mangle_idx_to_handle[mangle_idx].as_typed());
                    }

                    if let Some(handle) = stubs.name_to_handle(name) {
                        assert_eq!($expected_kind, handle.kind);
                        break 'a Some(handle.raw.as_typed());
                    }

                    None
                }
            };
        }

        let types = UniqueArenaMerger::new(&mut self.module.types, module.types, |req| {
            req.map(|types, span, ty| {
                // Attempt to map the type to its non-stubbed version
                if let Some(name) = &ty.name {
                    if let Some(handle) = dedup_name!(name, ExportKind::Types) {
                        return (MapResult::Dedup(handle), None);
                    }
                }

                // Otherwise, map it so that it can be integrated into the current arena.
                let mut ty = ty.fold(&folders!(a: types, b: &map_span));

                // If it has a name, mangle it.
                let demangle_to;
                if let Some(name) = &mut ty.name {
                    let demangled_name = name.clone();
                    let mangle_id = self.mangler.mangle_mut(name);
                    demangle_to = Some((demangled_name, mangle_id));
                } else {
                    demangle_to = None
                }

                (MapResult::Map(map_span(span), ty), demangle_to)
            })
            .post_map(
                |_types, _dest_arena, demangle_to, _src_handle, dest_handle| {
                    if let Some((demangled_name, mangle_id)) = demangle_to {
                        file.exports.insert(
                            demangled_name,
                            AnyNagaHandle {
                                kind: ExportKind::Types,
                                raw: RawNagaHandle::from_typed(dest_handle),
                            },
                        );
                        *self.mangle_idx_to_handle.entry(mangle_id) =
                            RawNagaHandle::from_typed(dest_handle);
                    }
                },
            );
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
            ($($name:ident as $kind:expr),*$(,)?) => {$(
                let mut $name = ArenaMerger::new(
                    &mut self.module.$name,
                    module.$name,
                    |_span, val| val.name.as_deref().and_then(|name| dedup_name!(name, $kind)),
                );
            )*};
        }

        new_stubs!(
            constants as ExportKind::Constants,
            overrides as ExportKind::Overrides,
            global_variables as ExportKind::GlobalVariables,
            functions as ExportKind::Functions,
        );

        // This arena doesn't need to be stubbed at all.
        let mut global_expressions = ArenaMerger::new(
            &mut self.module.global_expressions,
            module.global_expressions,
            |_, _| None,
        );

        // Let's fold the new (un-stubbed) values to properly integrate them into the new arena.
        macro_rules! apply_stubs {
            ($($name:ident as $kind:expr),*$(,)?) => {$(
                $name.apply(|$name, handle, span, mut val| {
                    if let Some(name) = &mut val.name {
                        file.exports.insert(name.clone(), AnyNagaHandle {
                            kind: $kind,
                            raw: $name.src_to_dest(handle).into(),
                        });

                        let mangle_idx = self.mangler.mangle_mut(name);
                        *self.mangle_idx_to_handle.entry(mangle_idx) =
                            RawNagaHandle::from_typed(handle);
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

        apply_stubs!(
            constants as ExportKind::Constants,
            overrides as ExportKind::Overrides,
            global_variables as ExportKind::GlobalVariables,
            functions as ExportKind::Functions,
        );

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

    pub fn gen_stubs<'s>(
        &self,
        imports: impl IntoIterator<Item = (FileHandle, &'s str, Option<&'s str>)>,
    ) -> ImportStubs {
        // Create a shaker for each arena.
        let sess = ArenaShakeSession::new();
        let mut constants = ArenaShaker::new(&sess, &self.module.constants);
        let mut overrides = ArenaShaker::new(&sess, &self.module.overrides);
        let mut global_variables = ArenaShaker::new(&sess, &self.module.global_variables);
        let mut global_expressions = ArenaShaker::new(&sess, &self.module.global_expressions);
        let mut functions = ArenaShaker::new(&sess, &self.module.functions);

        let mut types = UniqueArenaShaker::new(&self.module.types, (), &|types, span, val, ()| {
            (
                span,
                val.clone().fold(&folders!(
                    a: &|v: Span| v,
                    b: &types.folder(&|| ()),
                )),
            )
        });

        // Seed each shaker with their base set of imports. Record where these new names map to.
        let mut names_to_handle = FxStrMap::new();
        let mut exported_mangles = FxHashMap::default();

        for (file, orig_name, rename_to) in imports {
            let rename_to = rename_to.unwrap_or(orig_name);

            let file = &self.files[file];
            let Some(&handle) = file.exports.get(orig_name) else {
                panic!("failed to find import {orig_name:?}");
            };

            names_to_handle.insert(rename_to, handle);

            let mangled_name = match handle.kind {
                ExportKind::Types => {
                    types.include(handle.raw.as_typed(), || ());
                    self.module.types[handle.raw.as_typed()]
                        .name
                        .as_ref()
                        .unwrap()
                }
                ExportKind::Constants => {
                    constants.include(handle.raw.as_typed(), || ());
                    self.module.constants[handle.raw.as_typed()]
                        .name
                        .as_ref()
                        .unwrap()
                }
                ExportKind::Overrides => {
                    overrides.include(handle.raw.as_typed(), || ());
                    self.module.overrides[handle.raw.as_typed()]
                        .name
                        .as_ref()
                        .unwrap()
                }
                ExportKind::GlobalVariables => {
                    global_variables.include(handle.raw.as_typed(), || ());
                    self.module.global_variables[handle.raw.as_typed()]
                        .name
                        .as_ref()
                        .unwrap()
                }
                ExportKind::Functions => {
                    functions.include(handle.raw.as_typed(), || ());
                    self.module.functions[handle.raw.as_typed()]
                        .name
                        .as_ref()
                        .unwrap()
                }
            };

            exported_mangles.insert(
                try_demangle(mangled_name).unwrap().1,
                ExportedMangle {
                    mangle_len: mangled_name.len(),
                    new_name: rename_to.to_string().into_boxed_str(),
                },
            );
        }

        // Construct tree-shaken stub arenas using the seeded names of the previous step.
        sess.run(|| {
            constants.run(|_constants, span, val, ()| {
                (
                    span,
                    naga::Constant {
                        name: val.name.clone(),
                        ty: types.include(val.ty, || ()),
                        init: global_expressions.include(val.init, || ()),
                    },
                )
            });

            overrides.run(|_overrides, span, val, ()| {
                (
                    span,
                    naga::Override {
                        name: val.name.clone(),
                        id: val.id,
                        ty: types.include(val.ty, || ()),
                        init: val.init.map(|expr| global_expressions.include(expr, || ())),
                    },
                )
            });

            global_variables.run(|_global_variables, span, val, ()| {
                (
                    span,
                    naga::GlobalVariable {
                        name: val.name.clone(),
                        space: val.space,
                        binding: val.binding.clone(),
                        ty: types.include(val.ty, || ()),
                        init: val.init.map(|expr| global_expressions.include(expr, || ())),
                    },
                )
            });

            global_expressions.run(|global_expressions, span, val, ()| {
                (
                    span,
                    val.clone().fold(&folders!(
                        a: &constants.folder(&|| ()),
                        b: &overrides.folder(&|| ()),
                        c: &types.folder(&|| ()),
                        d: &global_expressions.folder(&|| ()),
                        e: &global_variables.folder(&|| ()),
                        f: &|_var: naga::Handle<naga::LocalVariable>| unreachable!("global expressions should not reference local variables"),
                        g: &functions.folder(&|| ()),
                    )),
                )
            });

            functions.run(|_functions, span, val, ()| {
                (
                    span,
                    naga::Function {
                        name: val.name.clone(),
                        arguments: val
                            .arguments
                            .iter()
                            .map(|arg| arg.clone().fold(&types.folder(&|| ())))
                            .collect(),
                        result: val
                            .result
                            .clone()
                            .map(|res| res.fold(&types.folder(&|| ()))),
                        local_variables: naga::Arena::new(),
                        expressions: naga::Arena::new(),
                        named_expressions: naga::FastIndexMap::default(),
                        body: naga::Block::from_vec(vec![naga::Statement::Kill]),
                    },
                )
            });
        });

        let module = naga::Module {
            types: types.finish(),
            special_types: naga::SpecialTypes::default(),
            constants: constants.finish(),
            overrides: overrides.finish(),
            global_variables: global_variables.finish(),
            global_expressions: global_expressions.finish(),
            functions: functions.finish(),
            entry_points: Vec::new(),
        };

        ImportStubs {
            module,
            names_to_handle,
            exported_mangles,
        }
    }

    pub fn full_module(&self) -> &naga::Module {
        &self.module
    }
}

#[derive(Debug)]
pub struct ImportStubs {
    module: naga::Module,
    names_to_handle: FxStrMap<AnyNagaHandle>,
    exported_mangles: FxHashMap<MangleIndex, ExportedMangle>,
}

#[derive(Debug)]
struct ExportedMangle {
    mangle_len: usize,
    new_name: Box<str>,
}

impl ImportStubs {
    pub fn empty() -> Self {
        ImportStubs {
            module: naga::Module::default(),
            names_to_handle: FxStrMap::new(),
            exported_mangles: FxHashMap::default(),
        }
    }

    pub fn module(&self) -> &naga::Module {
        &self.module
    }

    pub fn apply_names_to_stub_mut(&self, stub: &mut String) {
        replace_mangles(stub, |idx, out| {
            if let Some(rename) = self.exported_mangles.get(&idx) {
                out.replace_known_len(rename.mangle_len, &rename.new_name);
            }
        })
    }

    pub fn apply_names_to_stub(&self, mut stub: String) -> String {
        self.apply_names_to_stub_mut(&mut stub);
        stub
    }

    fn name_to_handle(&self, name: &str) -> Option<AnyNagaHandle> {
        self.names_to_handle.get(name).copied()
    }
}

// === File === //

#[derive(Default)]
struct File {
    exports: FxHashMap<String, AnyNagaHandle>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct AnyNagaHandle {
    kind: ExportKind,
    raw: RawNagaHandle,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum ExportKind {
    Types,
    Constants,
    Overrides,
    GlobalVariables,
    Functions,
}

// === Helpers === //

#[derive(Debug, Default)]
struct NameMangler(u64);

impl NameMangler {
    pub fn mangle_mut(&mut self, name: &mut String) -> MangleIndex {
        let idx = MangleIndex::from_raw(self.0);
        self.0 += 1;
        mangle_mut(name, idx);
        idx
    }
}