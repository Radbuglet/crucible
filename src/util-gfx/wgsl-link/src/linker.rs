use std::ops::Range;

use crucible_utils::{
    define_index,
    hash::{fx_hash_one, hashbrown::hash_map::RawEntryMut, FxHashMap},
    newtypes::{Index, IndexVec, LargeIndex},
    polyfill::{copy_hygiene, OptionExt},
};

use crate::merge::{
    folders, handle_from_usize, ArenaMerger, Foldable, FolderExt as _, MapResult, UniqueArenaMerger,
};

define_index! {
    pub struct FileHandle: u32;
    pub struct MangleIndex: u64;
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
    mangle_idx_to_handle: IndexVec<MangleIndex, usize>,
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
        imports: &ImportMap,
    ) -> FileHandle {
        let mut file = File::default();

        let map_span = |span: naga::Span| -> naga::Span {
            span.to_range().map_or(naga::Span::UNDEFINED, |v| {
                naga::Span::new(v.start as u32 + span_offset, v.end as u32 + span_offset)
            })
        };

        // Since everything depends on types, let's import those first.
        macro_rules! dedup_name {
            ($name:expr, $dedup_from:ident) => {
                'a: {
                    let name = $name;

                    if let Some((_, mangle_idx)) = NameMangler::try_demangle(name) {
                        break 'a Some(handle_from_usize(self.mangle_idx_to_handle[mangle_idx]));
                    }

                    if let Some(file) = imports.name_to_file(name) {
                        break 'a Some(self.files[file].$dedup_from[name]);
                    }

                    None
                }
            };
        }

        let types = UniqueArenaMerger::new(&mut self.module.types, module.types, |req| {
            req.map(|types, span, ty| {
                // Attempt to map the type to its non-stubbed version
                if let Some(name) = &ty.name {
                    if let Some(handle) = dedup_name!(name, types) {
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
                        file.types.insert(demangled_name, dest_handle);
                        *self.mangle_idx_to_handle.entry(mangle_id) = dest_handle.index();
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
            ($($name:ident),*$(,)?) => {$(
                let mut $name = ArenaMerger::new(
                    &mut self.module.$name,
                    module.$name,
                    |_span, val| val.name.as_deref().and_then(|name| dedup_name!(name, $name)),
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

                        let mangle_idx = self.mangler.mangle_mut(name);
                        *self.mangle_idx_to_handle.entry(mangle_idx) = handle.index();
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

    pub fn gen_stubs(&self, imports: &ImportMap) -> naga::Module {
        let mut module = self.module.clone();

        // TODO: Properly stub things instead of re-parsing the entire module
        for (file, name) in imports.imports() {
            let file = &self.files[file];

            if let Some(&handle) = file.types.get(name) {
                let mut ty = self.module.types[handle].clone();
                NameMangler::demangle_mut(ty.name.as_mut().unwrap());
                module.types.replace(handle, ty);
                continue;
            }

            macro_rules! rename_arenas {
                ($($name:ident),*$(,)?) => {$(
                    if let Some(&handle) = file.$name.get(name) {
                        NameMangler::demangle_mut(module.$name[handle].name.as_mut().unwrap());
                        continue;
                    }
                )*};
            }

            rename_arenas!(constants, overrides, global_variables, functions);
        }

        module
    }

    pub fn module(&self) -> &naga::Module {
        &self.module
    }
}

#[derive(Debug, Default)]
pub struct ImportMap {
    names: String,
    imports: Vec<(FileHandle, Range<usize>)>,
    name_to_file: FxHashMap<Range<usize>, FileHandle>,
}

impl ImportMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, file: FileHandle, name: &str) {
        let start = self.names.len();
        self.names.push_str(name);
        let range = start..self.names.len();

        self.imports.push((file, range.clone()));

        let hash = fx_hash_one(name);

        let RawEntryMut::Vacant(entry) = self
            .name_to_file
            .raw_entry_mut()
            .from_hash(hash, |range| &self.names[range.clone()] == name)
        else {
            panic!("{name:?} added to import map more than once");
        };

        entry.insert_with_hasher(hash, range, file, |v| fx_hash_one(&self.names[v.clone()]));
    }

    pub fn imports(&self) -> impl Iterator<Item = (FileHandle, &str)> + '_ {
        self.imports
            .iter()
            .map(|(file, range)| (*file, &self.names[range.clone()]))
    }

    pub fn name_to_file(&self, name: &str) -> Option<FileHandle> {
        let hash = fx_hash_one(name);
        self.name_to_file
            .raw_entry()
            .from_hash(hash, |range| &self.names[range.clone()] == name)
            .map(|(_k, v)| *v)
    }
}

impl<'a> FromIterator<(FileHandle, &'a str)> for ImportMap {
    fn from_iter<T: IntoIterator<Item = (FileHandle, &'a str)>>(iter: T) -> Self {
        let mut map = ImportMap::new();
        for (file, name) in iter {
            map.push(file, name);
        }
        map
    }
}

#[derive(Debug, Default)]
struct NameMangler(u64);

impl NameMangler {
    const MANGLE_SEP: &'static str = "_MANGLE_";

    pub fn mangle_mut(&mut self, name: &mut String) -> MangleIndex {
        use std::fmt::Write as _;
        let idx = MangleIndex::from_raw(self.0);
        write!(name, "{}{:x}_", Self::MANGLE_SEP, self.0).unwrap();
        self.0 += 1;
        idx
    }

    pub fn try_demangle(name: &str) -> Option<(&str, MangleIndex)> {
        let idx = name.rfind(Self::MANGLE_SEP)?;

        let left = &name[..idx];

        let right = &name[idx..][Self::MANGLE_SEP.len()..];
        let right = &right[..(right.len() - 1)];
        let right = MangleIndex::from_usize(usize::from_str_radix(right, 16).unwrap());

        Some((left, right))
    }

    pub fn demangle_mut(name: &mut String) {
        name.truncate(name.rfind(Self::MANGLE_SEP).expect("name was not mangled"))
    }
}
