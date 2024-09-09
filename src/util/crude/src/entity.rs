use std::{
    borrow::Cow,
    fmt,
    num::NonZeroU64,
    sync::{
        atomic::{AtomicU64, Ordering::*},
        OnceLock,
    },
};

use crucible_utils::hash::{xorshift64_raw, NopBuildHasher};

// === Debug Labels === //

fn debug_labels() -> &'static dashmap::DashMap<Entity, Cow<'static, str>, NopBuildHasher> {
    static DEBUG_LABELS: OnceLock<dashmap::DashMap<Entity, Cow<'static, str>, NopBuildHasher>> =
        OnceLock::new();

    DEBUG_LABELS.get_or_init(Default::default)
}

pub(crate) fn set_debug_label(entity: Entity, label: impl Into<Cow<'static, str>>) {
    debug_labels().insert(entity, label.into());
}

pub(crate) fn unset_debug_label(entity: Entity) {
    debug_labels().remove(&entity);
}

// === Reservations === //

static ENTITY_ALLOC: AtomicU64 = AtomicU64::new(xorshift64_raw(1));

pub(crate) fn reserve_entity() -> Entity {
    let id = unsafe {
        NonZeroU64::new_unchecked(
            ENTITY_ALLOC
                .fetch_update(Relaxed, Relaxed, |v| Some(xorshift64_raw(v)))
                .unwrap_unchecked(),
        )
    };

    Entity { id }
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

        if let Some(label) = debug_labels().get(&self) {
            f.field(&label);
        }

        f.finish()
    }
}
