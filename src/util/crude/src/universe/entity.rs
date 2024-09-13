use std::{borrow::Cow, fmt, num::NonZeroU64, sync::OnceLock};

use crucible_utils::{
    fmt::CowDisplay,
    hash::{NopBuildHasher, Xorshift, XorshiftPool},
};

// === EntityAllocator === //

static POOL: XorshiftPool = XorshiftPool::new();

pub struct EntityAllocator {
    local: Xorshift,
}

impl fmt::Debug for EntityAllocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entity").finish_non_exhaustive()
    }
}

impl EntityAllocator {
    pub const fn new() -> Self {
        Self {
            local: Xorshift::new_empty(),
        }
    }

    pub fn spawn(&mut self, label: impl CowDisplay) -> Entity {
        let id = unsafe { NonZeroU64::new_unchecked(POOL.gen_local(&mut self.local)) };
        let entity = Entity { id };
        entity.set_debug_label(label.fmt_cow());
        entity
    }
}

impl Drop for EntityAllocator {
    fn drop(&mut self) {
        POOL.dealloc_local(self.local.clone());
    }
}

// === Entity === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Entity {
    pub(crate) id: NonZeroU64,
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("Entity");

        f.field(&self.id);

        if let Some(label) = Self::debug_labels().get(&self) {
            f.field(&label);
        }

        f.finish()
    }
}

impl Entity {
    fn debug_labels() -> &'static dashmap::DashMap<Entity, Cow<'static, str>, NopBuildHasher> {
        static DEBUG_LABELS: OnceLock<dashmap::DashMap<Entity, Cow<'static, str>, NopBuildHasher>> =
            OnceLock::new();

        DEBUG_LABELS.get_or_init(Default::default)
    }

    pub fn set_debug_label(self, label: impl Into<Cow<'static, str>>) {
        Self::debug_labels().insert(self, label.into());
    }

    pub fn unset_debug_label(self) {
        Self::debug_labels().remove(&self);
    }
}
