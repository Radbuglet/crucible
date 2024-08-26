use std::ops::Range;

use crucible_utils::{
    define_index,
    hash::{hashbrown::hash_map, FxHashMap, FxStrMap},
    newtypes::{IndexVec, LargeIndex as _},
    polyfill::{copy_hygiene, OptionExt},
};

use crate::{
    mangle::{mangle_mut, replace_mangles, try_demangle, MangleIndex},
    module::{
        map::{Map as _, MapCombinatorsExt, MapFn, MapNever},
        map_naga::{
            map_naga_constant, map_naga_expression, map_naga_function, map_naga_global_variable,
            map_naga_override, map_naga_type,
        },
    },
};

use super::{
    map::{map_collection, map_option, MapIdentity},
    map_naga::{map_naga_function_arg, map_naga_function_result},
    merge::{ArenaMerger, MapResult, RawNagaHandle, UniqueArenaMerger},
    shake::{ArenaShakeSession, ArenaShaker, UniqueArenaShaker},
};

// === ModuleLinker === //

define_index! {
    pub struct ModuleHandle: u32;
}

#[derive(Default)]
pub struct ModuleLinker {
    // The entire link context is contained within this module. We apply tree-shaking to produce
    // the final module to allow the linker to be effectively reused between compilation sessions.
    module: naga::Module,

    // Maps individual files and their exports into handles into the giant module we're constructing.
    files: IndexVec<ModuleHandle, LinkedModule>,

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
        parent_span: Range<u32>,
    ) -> ModuleHandle {
        let mut file = LinkedModule::default();
        let parent_span_start = parent_span.start;
        let parent_span_len = parent_span.end - parent_span.start;

        let map_span = MapFn(|span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                debug_assert!(v.end as u32 <= parent_span_len);
                naga::Span::new(
                    parent_span_start + v.start as u32,
                    parent_span_start + v.end as u32,
                )
            })
        });

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
                let mut ty = map_naga_type(ty, &types.and(&map_span));

                // If it has a name, mangle it.
                let demangle_to;
                if let Some(name) = &mut ty.name {
                    let demangled_name = name.clone();
                    let mangle_id = self.mangler.mangle_mut(name);
                    demangle_to = Some((demangled_name, mangle_id));
                } else {
                    demangle_to = None
                }

                (MapResult::Map(map_span.map(span), ty), demangle_to)
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
            ($(($name:ident, $mapper:expr, $kind:expr)),*$(,)?) => {$(
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
                        map_span.map(span),
                        ($mapper)(
                            val,
                            &copy_hygiene!($name, &constants)
                                .and(copy_hygiene!($name, &overrides))
                                .and(copy_hygiene!($name, &types))
                                .and(copy_hygiene!($name, &global_variables))
                                .and(copy_hygiene!($name, &functions))
                                .and(copy_hygiene!($name, &map_span))
                                .and(copy_hygiene!($name, &global_expressions)),
                        ),
                    )
                });
            )*};
        }

        #[rustfmt::skip]
        apply_stubs!(
            (constants, map_naga_constant, ExportKind::Constants),
            (overrides, map_naga_override, ExportKind::Overrides),
            (global_variables, map_naga_global_variable, ExportKind::GlobalVariables),
            (functions, map_naga_function, ExportKind::Functions),
        );

        global_expressions.apply(|global_expressions, _handle, span, val| {
            // (expressions do not have names to mangle)

            (
                map_span.map(span),
                map_naga_expression(
                    val,
                    &(&constants)
                        .and(&overrides)
                        .and(&types)
                        .and(&global_expressions)
                        .and(&global_variables)
                        .and(&MapNever::<naga::Handle<naga::LocalVariable>>::new())
                        .and(&functions)
                        .and(&map_span),
                ),
            )
        });

        // Finally, let's import the entry-points.
        let first_entry_point = self.module.entry_points.len();
        self.module
            .entry_points
            .extend(module.entry_points.into_iter().map(|entry| {
                naga::EntryPoint {
                    name: entry.name,
                    stage: entry.stage,
                    early_depth_test: entry.early_depth_test,
                    workgroup_size: entry.workgroup_size,
                    function: map_naga_function(
                        entry.function,
                        &(&constants)
                            .and(&overrides)
                            .and(&types)
                            .and(&global_variables)
                            .and(&functions)
                            .and(&map_span)
                            .and(&global_expressions),
                    ),
                }
            }));

        file.entry_point_range = first_entry_point..self.module.entry_points.len();

        self.files.push(file)
    }

    pub fn gen_stubs<'s, M>(
        &self,
        imports: impl IntoIterator<Item = LinkerImport<'s, M>>,
        mut handle_err: impl FnMut(LinkerImportError<'_, M>),
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
                map_naga_type(
                    val.clone(),
                    &MapIdentity::<naga::Span>::new().and(types.folder(&|| ())),
                ),
            )
        });

        // Seed each shaker with their base set of imports. Record where these new names map to.
        let mut names_to_handle = FxStrMap::new();
        let mut exported_mangles = FxHashMap::default();
        let mut exported_mangle_directives = FxHashMap::<MangleIndex, LinkerImport<M>>::default();
        let mut exported_mangle_targets = FxStrMap::<MangleIndex>::new();

        for import in imports {
            let rename_to = import.rename_to.unwrap_or(import.orig_name);

            let file = &self.files[import.file];
            let Some(&handle) = file.exports.get(import.orig_name) else {
                handle_err(LinkerImportError::UnknownImport(&import));
                continue;
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

            let demangle_idx = try_demangle(mangled_name).unwrap().1;

            // Ensure that the rename target hasn't been used yet.
            if let Some(first) = exported_mangle_targets.get(rename_to) {
                handle_err(LinkerImportError::DuplicateDestinations(
                    &exported_mangle_directives[first],
                    &import,
                ));
                continue;
            }
            exported_mangle_targets.insert(rename_to, demangle_idx);

            // Ensure that the source symbol hasn't been used yet.
            match exported_mangle_directives.entry(demangle_idx) {
                hash_map::Entry::Occupied(first) => {
                    handle_err(LinkerImportError::DuplicateSources(first.get(), &import));
                    continue;
                }
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(import);
                }
            }

            exported_mangles.insert(
                demangle_idx,
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
                    map_naga_expression(
                        val.clone(),
                        &constants
                            .folder(&|| ())
                            .and(overrides.folder(&|| ()))
                            .and(types.folder(&|| ()))
                            .and(global_expressions.folder(&|| ()))
                            .and(global_variables.folder(&|| ()))
                            .and(functions.folder(&|| ()))
                            .and(MapNever::<naga::Handle<naga::LocalVariable>>::new())
                            .and(MapIdentity::<naga::Span>::new()),
                    ),
                )
            });

            functions.run(|_functions, span, val, ()| {
                (
                    span,
                    naga::Function {
                        name: val.name.clone(),
                        arguments: map_collection(
                            val.arguments.clone(),
                            &map_naga_function_arg.complete(types.folder(&|| ())),
                        ),
                        result: map_option(
                            val.result.clone(),
                            &map_naga_function_result.complete(types.folder(&|| ())),
                        ),
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

    pub fn shake_module(&self, modules: impl IntoIterator<Item = ModuleHandle>) -> naga::Module {
        // Create a shaker for each arena.
        let sess = ArenaShakeSession::new();
        let mut types =
            UniqueArenaShaker::new(&self.module.types, (), &|types, span, value, ()| {
                let value = map_naga_type(
                    value.clone(),
                    &types.folder(&|| ()).and(MapIdentity::<naga::Span>::new()),
                );

                (span, value)
            });

        let mut constants = ArenaShaker::new(&sess, &self.module.constants);
        let mut overrides = ArenaShaker::new(&sess, &self.module.overrides);
        let mut global_variables = ArenaShaker::new(&sess, &self.module.global_variables);
        let mut global_expressions = ArenaShaker::new(&sess, &self.module.global_expressions);
        let mut functions = ArenaShaker::new(&sess, &self.module.functions);
        let mut entry_points = Vec::new();

        // Seed each shaker with the modules' exports.
        for module in modules {
            let module = &self.files[module];

            for (_name, export) in module.exports.iter() {
                match export.kind {
                    ExportKind::Types => {
                        types.include(export.raw.as_typed(), || ());
                    }
                    ExportKind::Constants => {
                        constants.include(export.raw.as_typed(), || ());
                    }
                    ExportKind::Overrides => {
                        overrides.include(export.raw.as_typed(), || ());
                    }
                    ExportKind::GlobalVariables => {
                        global_variables.include(export.raw.as_typed(), || ());
                    }
                    ExportKind::Functions => {
                        functions.include(export.raw.as_typed(), || ());
                    }
                }
            }

            entry_points.extend(
                self.module.entry_points[module.entry_point_range.clone()]
                    .iter()
                    .map(|entry| naga::EntryPoint {
                        name: entry.name.clone(),
                        stage: entry.stage,
                        early_depth_test: entry.early_depth_test,
                        workgroup_size: entry.workgroup_size,
                        function: map_naga_function(
                            entry.function.clone(),
                            &constants
                                .folder(&|| ())
                                .and(overrides.folder(&|| ()))
                                .and(types.folder(&|| ()))
                                .and(global_variables.folder(&|| ()))
                                .and(functions.folder(&|| ()))
                                .and(MapIdentity::<naga::Span>::new()),
                        ),
                    }),
            );
        }

        // Construct the tree-shaken arenas.
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
                    map_naga_expression(
                        val.clone(),
                        &constants
                            .folder(&|| ())
                            .and(overrides.folder(&|| ()))
                            .and(types.folder(&|| ()))
                            .and(global_expressions.folder(&|| ()))
                            .and(global_variables.folder(&|| ()))
                            .and(&functions.folder(&|| ()))
                            .and(MapNever::<naga::Handle<naga::LocalVariable>>::new())
                            .and(MapIdentity::<naga::Span>::new()),
                    ),
                )
            });

            functions.run(|functions, span, val, ()| {
                (
                    span,
                    map_naga_function(
                        val.clone(),
                        &constants
                            .folder(&|| ())
                            .and(overrides.folder(&|| ()))
                            .and(types.folder(&|| ()))
                            .and(global_variables.folder(&|| ()))
                            .and(functions.folder(&|| ()))
                            .and(MapIdentity::<naga::Span>::new()),
                    ),
                )
            });
        });

        naga::Module {
            types: types.finish(),
            special_types: naga::SpecialTypes::default(),
            constants: constants.finish(),
            overrides: overrides.finish(),
            global_variables: global_variables.finish(),
            global_expressions: global_expressions.finish(),
            functions: functions.finish(),
            entry_points,
        }
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

#[derive(Debug, Clone)]
pub struct LinkerImport<'s, M> {
    pub file: ModuleHandle,
    pub orig_name: &'s str,
    pub rename_to: Option<&'s str>,
    pub meta: M,
}

#[derive(Debug, Clone)]
pub enum LinkerImportError<'a, M> {
    UnknownImport(&'a LinkerImport<'a, M>),
    DuplicateSources(&'a LinkerImport<'a, M>, &'a LinkerImport<'a, M>),
    DuplicateDestinations(&'a LinkerImport<'a, M>, &'a LinkerImport<'a, M>),
}

// === LinkedModule === //

#[derive(Default)]
struct LinkedModule {
    exports: FxHashMap<String, AnyNagaHandle>,
    // TODO: Allow entry points to be imported?
    entry_point_range: Range<usize>,
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
