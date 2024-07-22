//! Utilities for merging `naga` arenas.

use std::{
    any::{type_name, Any},
    hash,
    num::NonZeroU32,
};

use naga::{Arena, Handle, Span, UniqueArena};

// === Helpers === //

fn handle_from_usize<T>(value: usize) -> Handle<T> {
    let value = u32::try_from(value)
        .ok()
        .filter(|&v| v != u32::MAX)
        .expect("handle too big");
    let value = NonZeroU32::new(value + 1).unwrap();

    unsafe {
        // FIXME: Safety: Nope. This is super unstable.
        std::mem::transmute(value)
    }
}

// === ArenaMerger === //

pub struct ArenaMerger<'a, T> {
    dest_arena: &'a mut Arena<T>,
    src_arena: Option<Arena<T>>,

    // The index of the first handle in destination arena imported from the source arena.
    dest_alloc_start: usize,

    // A map from source handles to destination handles.
    src_to_dest: Vec<Handle<T>>,

    // A map from destination handles—offset by `dest_alloc_start`—to source handles.
    dest_to_src: Vec<Handle<T>>,
}

impl<'a, T> ArenaMerger<'a, T> {
    pub fn new(
        dest_arena: &'a mut Arena<T>,
        src_arena: Arena<T>,
        mut dedup: impl FnMut(Span, &T) -> Option<Handle<T>>,
    ) -> Self {
        let dest_offset = dest_arena.len();
        let mut src_to_dest = Vec::with_capacity(src_arena.len());
        let mut dest_to_src = Vec::new();

        let mut index_alloc = dest_offset;

        for (src_handle, value) in src_arena.iter() {
            let span = src_arena.get_span(src_handle);

            if let Some(map_to) = dedup(span, value) {
                src_to_dest.push(map_to);
            } else {
                src_to_dest.push(handle_from_usize(index_alloc));
                dest_to_src.push(src_handle);
                index_alloc += 1;
            }
        }

        Self {
            dest_arena,
            src_arena: Some(src_arena),
            dest_alloc_start: dest_offset,
            src_to_dest,
            dest_to_src,
        }
    }

    pub fn src_to_dest(&self, src_handle: Handle<T>) -> Handle<T> {
        self.src_to_dest[src_handle.index()]
    }

    pub fn dest_to_src(&self, dest_handle: Handle<T>) -> Option<Handle<T>> {
        dest_handle
            .index()
            .checked_sub(self.dest_alloc_start)
            .map(|idx| self.dest_to_src[idx])
    }

    pub fn lookup_src(&self, src_handle: Handle<T>) -> &T {
        if let Some(src) = &self.src_arena {
            &src[src_handle]
        } else {
            &self.dest_arena[self.src_to_dest(src_handle)]
        }
    }

    pub fn lookup_src_mut(&mut self, src_handle: Handle<T>) -> &mut T {
        let dest_handle = self.src_to_dest(src_handle);

        if let Some(src) = &mut self.src_arena {
            &mut src[src_handle]
        } else {
            &mut self.dest_arena[dest_handle]
        }
    }

    pub fn lookup_dest(&self, dest_handle: Handle<T>) -> &T {
        let src_handle = self.dest_to_src(dest_handle);

        if let (Some(src_handle), Some(src)) = (src_handle, &self.src_arena) {
            &src[src_handle]
        } else {
            &self.dest_arena[dest_handle]
        }
    }

    pub fn lookup_dest_mut(&mut self, dest_handle: Handle<T>) -> &mut T {
        let src_handle = self.dest_to_src(dest_handle);

        if let (Some(src_handle), Some(src)) = (src_handle, &mut self.src_arena) {
            &mut src[src_handle]
        } else {
            &mut self.dest_arena[dest_handle]
        }
    }

    pub fn apply(&mut self, mut adjust: impl FnMut(&Self, Span, T) -> (Span, T)) {
        let mut inserted_arena = self
            .src_arena
            .take()
            .expect("cannot call `apply` on a given `ArenaMerger` more than once");

        let mut included_handle_iter = self.dest_to_src.iter().copied().peekable();

        for (handle, value, span) in inserted_arena.drain() {
            if included_handle_iter.peek() == Some(&handle) {
                let _ = included_handle_iter.next();
            } else {
                continue;
            }

            let (span, value) = adjust(self, span, value);
            self.dest_arena.append(value, span);
        }
    }
}

impl<T> Folder<Handle<T>> for ArenaMerger<'_, T> {
    fn fold(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

// === UniqueArenaMerger === //

pub struct UniqueArenaMerger<T> {
    map: Vec<Handle<T>>,
}

impl<T> UniqueArenaMerger<T> {
    pub fn new(
        dest_arena: &mut UniqueArena<T>,
        src_arena: UniqueArena<T>,
        mut map: impl FnMut(&UniqueArenaMerger<T>, Span, T) -> MapResult<T>,
    ) -> Self
    where
        T: Eq + hash::Hash + Clone,
    {
        // In its current form, Naga never emits recursive types and, indeed, ensures that `UniqueArena`
        // is properly toposorted. Hence, this algorithm is safe.

        let mut mapper = UniqueArenaMerger { map: Vec::new() };

        for (handle, value) in src_arena.iter() {
            let span = src_arena.get_span(handle);
            let value = value.clone();

            mapper.map.push(match map(&mapper, span, value) {
                MapResult::Map(span, value) => dest_arena.insert(value, span),
                MapResult::Dedup(handle) => handle,
            });
        }

        mapper
    }

    pub fn src_to_dest(&self, src_arena: Handle<T>) -> Handle<T> {
        self.map[src_arena.index()]
    }
}

impl<T> Folder<Handle<T>> for UniqueArenaMerger<T> {
    fn fold(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

pub enum MapResult<T> {
    Map(Span, T),
    Dedup(Handle<T>),
}

// === Foldable Core === //

// Trait definitions
pub trait Folder<T> {
    fn fold(&self, value: T) -> T;
}

impl<F, T> Folder<T> for F
where
    F: Fn(T) -> T,
{
    fn fold(&self, value: T) -> T {
        self(value)
    }
}

pub trait Foldable<F> {
    fn fold(self, f: &F) -> Self;
}

// CompositeFolder
pub struct CompositeFolder<F>(pub F)
where
    F: Fn(&mut FoldReq<'_>);

impl<F, T: 'static> Folder<T> for CompositeFolder<F>
where
    F: Fn(&mut FoldReq<'_>),
{
    fn fold(&self, value: T) -> T {
        let mut value = Some(value);
        let mut req = FoldReq {
            value: &mut value,
            processed: false,
        };

        self.0(&mut req);

        assert!(
            req.processed,
            "no folder specified for `{}`",
            type_name::<T>()
        );
        value.unwrap()
    }
}

pub struct FoldReq<'a> {
    value: &'a mut dyn Any,
    processed: bool,
}

impl FoldReq<'_> {
    pub fn fold<T: 'static>(&mut self, f: &(impl ?Sized + Folder<T>)) -> &mut Self {
        if let Some(target) = self.value.downcast_mut::<Option<T>>() {
            assert!(
                !self.processed,
                "more than one folder specified for `{}`",
                type_name::<T>()
            );
            *target = Some(f.fold(target.take().unwrap()));
            self.processed = true;
        }

        self
    }
}

// Macro
macro_rules! folders {
    ($($name:ident: $folder:expr),+$(,)?) => {{
        #[allow(non_camel_case_types)]
        fn produce<'a, $($name: 'static),*>($($name: &'a (impl ?Sized + $crate::merge::Folder<$name>),)*)
            -> impl 'a $(+ $crate::merge::Folder<$name>)*
        {
            $crate::merge::CompositeFolder(|req| {
                req $(.fold($name))* ;
            })
        }

        produce($($folder),*)
    }};
}

pub(crate) use folders;

// === Foldable Implementations === //

impl<F> Foldable<F> for naga::Type
where
    F: Folder<Handle<naga::Type>> + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        use naga::TypeInner::*;

        naga::Type {
            name: self.name,
            inner: match self.inner {
                value @ Scalar(_)
                | value @ Vector { .. }
                | value @ Matrix { .. }
                | value @ Atomic(_)
                | value @ ValuePointer { .. }
                | value @ Image { .. }
                | value @ Sampler { .. }
                | value @ AccelerationStructure
                | value @ RayQuery => value,
                Pointer { base, space } => Pointer {
                    base: f.fold(base),
                    space,
                },
                Array { base, size, stride } => Array {
                    base: f.fold(base),
                    size,
                    stride,
                },
                Struct { members, span } => Struct {
                    members: members
                        .into_iter()
                        .map(|member| naga::StructMember {
                            name: member.name,
                            ty: f.fold(member.ty),
                            binding: member.binding,
                            offset: member.offset,
                        })
                        .collect(),
                    // TODO: How do we map this??
                    span,
                },
                BindingArray { base, size } => BindingArray {
                    base: f.fold(base),
                    size,
                },
            },
        }
    }
}
