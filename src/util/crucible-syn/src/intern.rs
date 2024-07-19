use core::fmt;
use std::{cell::UnsafeCell, hash::BuildHasher as _, ptr::NonNull};

use crucible_utils::{
    define_index,
    hash::{hashbrown::hash_map, FxBuildHasher, FxHashMap},
    newtypes::IndexVec,
};

// === Core === //

#[derive(Default)]
pub struct Interner {
    bump: bumpalo::Bump,
    inner: UnsafeCell<InternerInner>,
}

#[derive(Default)]
struct InternerInner {
    intern_map: FxHashMap<InternKey, Intern>,
    interns: IndexVec<Intern, NonNull<str>>,
}

struct InternKey {
    text: NonNull<str>,
    hash: u64,
}

impl fmt::Debug for Interner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner").finish_non_exhaustive()
    }
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    unsafe fn intern_inner(
        &self,
        text: &str,
        alloc: impl FnOnce(&bumpalo::Bump) -> NonNull<str>,
    ) -> Intern {
        let hash = FxBuildHasher::default().hash_one(text);
        let inner = unsafe { &mut *self.inner.get() };
        let entry = inner.intern_map.raw_entry_mut().from_hash(hash, |key| {
            key.hash == hash && unsafe { key.text.as_ref() == text }
        });

        match entry {
            hash_map::RawEntryMut::Occupied(entry) => *entry.get(),
            hash_map::RawEntryMut::Vacant(entry) => {
                let text = alloc(&self.bump);
                let intern = inner.interns.push(text);
                entry.insert_with_hasher(hash, InternKey { text, hash }, intern, |v| v.hash);
                intern
            }
        }
    }

    pub fn intern_static(&self, text: &'static str) -> Intern {
        unsafe { self.intern_inner(text, |_bump| NonNull::from(text)) }
    }

    pub fn intern(&self, text: &str) -> Intern {
        unsafe { self.intern_inner(text, |bump| NonNull::from(&*bump.alloc_str(text))) }
    }

    pub fn lookup(&self, intern: Intern) -> &str {
        unsafe { (*self.inner.get()).interns[intern].as_ref() }
    }

    pub fn reset(&mut self) {
        let inner = self.inner.get_mut();
        inner.interns.raw.clear();
        inner.intern_map.clear();
        self.bump.reset();
    }
}

define_index! {
    pub struct Intern: u32;
}

// === Dependency Injection === //

autoken::cap! {
    pub InternerCap = Interner;
}

impl Intern {
    pub fn new(text: &str) -> Self {
        autoken::cap!(ref InternerCap).intern(text)
    }

    pub fn new_static(text: &'static str) -> Self {
        autoken::cap!(ref InternerCap).intern_static(text)
    }

    pub fn as_str<'a>(self) -> &'a str {
        autoken::tie!('a => ref InternerCap);
        autoken::cap!(ref InternerCap).lookup(self)
    }
}
