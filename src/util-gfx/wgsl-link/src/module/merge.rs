//! Utilities for merging `naga` arenas.

use std::{hash, num::NonZeroU32};

use crucible_utils::{
    define_index,
    multi_closure::multi_closure,
    newtypes::{Index as _, IndexVec, LargeIndex as _},
};
use naga::{Arena, Handle, Span, UniqueArena};

use super::map::Map;

// === Helpers === //

define_index! {
    #[derive(Default)]
    pub struct RawNagaHandle: u32;
}

impl<T> From<Handle<T>> for RawNagaHandle {
    fn from(value: Handle<T>) -> Self {
        Self::from_typed(value)
    }
}

impl RawNagaHandle {
    pub fn from_typed<T>(handle: Handle<T>) -> Self {
        Self::from_usize(handle.index())
    }

    pub fn as_typed<T>(self) -> Handle<T> {
        assert!(self.as_raw() != u32::MAX, "handle too big");
        let value = NonZeroU32::new(self.as_raw() + 1).unwrap();

        unsafe {
            // FIXME: Safety: Nope. This is super unstable.
            std::mem::transmute(value)
        }
    }
}

// === ArenaMerger === //

pub struct ArenaMerger<'a, T> {
    dest_arena: &'a mut Arena<T>,
    src_arena: Option<Arena<T>>,

    // The index of the first handle in destination arena imported from the source arena.
    dest_alloc_start: usize,

    // A map from source handles to destination handles.
    src_to_dest: IndexVec<RawNagaHandle, Handle<T>>,

    // A map from destination handles—offset by `dest_alloc_start`—to source handles.
    dest_to_src: IndexVec<RawNagaHandle, Handle<T>>,
}

impl<'a, T> ArenaMerger<'a, T> {
    pub fn new(
        dest_arena: &'a mut Arena<T>,
        src_arena: Arena<T>,
        mut dedup: impl FnMut(Span, &T) -> Option<Handle<T>>,
    ) -> Self {
        let dest_offset = dest_arena.len();
        let mut src_to_dest = IndexVec::from_raw(Vec::with_capacity(src_arena.len()));
        let mut dest_to_src = IndexVec::new();

        let mut index_alloc = dest_offset;

        for (src_handle, value) in src_arena.iter() {
            let span = src_arena.get_span(src_handle);

            if let Some(map_to) = dedup(span, value) {
                src_to_dest.push(map_to);
            } else {
                src_to_dest.push(RawNagaHandle::from_usize(index_alloc).as_typed());
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
        self.src_to_dest[RawNagaHandle::from_typed(src_handle)]
    }

    pub fn dest_to_src(&self, dest_handle: Handle<T>) -> Option<Handle<T>> {
        dest_handle
            .index()
            .checked_sub(self.dest_alloc_start)
            .map(|idx| self.dest_to_src[RawNagaHandle::from_usize(idx)])
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

    pub fn apply(&mut self, mut adjust: impl FnMut(&Self, Handle<T>, Span, T) -> (Span, T)) {
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

            let (span, value) = adjust(self, handle, span, value);
            self.dest_arena.append(value, span);
        }
    }
}

impl<T> Map<Handle<T>, ()> for ArenaMerger<'_, T> {
    fn map(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

// === UniqueArenaMerger === //

pub struct UniqueArenaMerger<T> {
    map: Vec<Handle<T>>,
}

impl<T> UniqueArenaMerger<T> {
    pub fn new<M>(
        dest_arena: &mut UniqueArena<T>,
        src_arena: UniqueArena<T>,
        mut map: impl FnMut(UniqueArenaMapRequest<T, M>),
    ) -> Self
    where
        T: Eq + hash::Hash + Clone,
    {
        // In its current form, Naga never emits recursive types and, indeed, ensures that `UniqueArena`
        // is properly toposorted. Hence, this algorithm is safe.

        let mut mapper = UniqueArenaMerger { map: Vec::new() };

        for (src_handle, value) in src_arena.iter() {
            let span = src_arena.get_span(src_handle);
            let value = value.clone();

            let (map_res, map_meta) =
                UniqueArenaMapRequest::call_map((&mapper, span, value), &mut map);

            let dest_handle = match map_res {
                MapResult::Map(span, value) => dest_arena.insert(value, span),
                MapResult::Dedup(handle) => handle,
            };
            mapper.map.push(dest_handle);

            UniqueArenaMapRequest::call_post_map(
                (&mapper, dest_arena, map_meta, src_handle, dest_handle),
                &mut map,
            );
        }

        mapper
    }

    pub fn src_to_dest(&self, src_arena: Handle<T>) -> Handle<T> {
        self.map[src_arena.index()]
    }
}

impl<T> Map<Handle<T>, ()> for UniqueArenaMerger<T> {
    fn map(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

multi_closure! {
    pub enum UniqueArenaMapRequest<'a, T, M> {
        map(&'a UniqueArenaMerger<T>, Span, T) -> (MapResult<T>, M),
        post_map(&'a UniqueArenaMerger<T>, &'a UniqueArena<T>, M, Handle<T>, Handle<T>),
    }
}

pub enum MapResult<T> {
    Map(Span, T),
    Dedup(Handle<T>),
}
