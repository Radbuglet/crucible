use std::{
    any::Any,
    fmt,
    ops::Deref,
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc, OnceLock, RwLock,
    },
};

use bevy_autoken::random_component;
use derive_where::derive_where;
use hash_util::{fx_hash_one, hashbrown::hash_map, FxHashMap, ManyToOwned};
use smallbox::{smallbox, SmallBox};

// === AssetManager === //

#[derive(Default)]
pub struct AssetManager {
    assets: RwLock<FxHashMap<AssetKey, AssetValue>>,
}

random_component!(AssetManager);

struct AssetKey {
    hash: u64,
    loader_ptr: usize,
    value: SmallBox<dyn Any + Send + Sync, [u64; 2]>,
}

struct AssetValue {
    deletion_candidate: AtomicBool,
    value: Arc<dyn Any + Send + Sync>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<C, A, R>(&self, cx: C, args: A, loader: fn(&Self, C, A) -> R) -> Asset<R>
    where
        A: ManyToOwned,
        A::Owned: 'static + Send + Sync,
        R: 'static + Send + Sync,
    {
        let asset = self.load_inner::<A, R>(args, loader as usize);
        let inner = NonNull::from(asset.get_or_init(|| loader(self, cx, args)));

        Asset {
            _asset: asset,
            inner,
        }
    }

    pub fn try_reclaim(&mut self) {
        self.assets.get_mut().unwrap().retain(|_, v| {
            let deletion_candidate = v.deletion_candidate.get_mut();

            if Arc::strong_count(&v.value) == 1 {
                let should_delete = !*deletion_candidate;
                *deletion_candidate = true;
                should_delete
            } else {
                *deletion_candidate = false;
                true
            }
        });
    }

    fn load_inner<A, R>(&self, args: A, loader_ptr: usize) -> Arc<OnceLock<R>>
    where
        A: ManyToOwned,
        A::Owned: 'static + Send + Sync,
        R: 'static + Send + Sync,
    {
        let hash = fx_hash_one(args);
        let check_candidate = |candidate: &AssetKey| -> bool {
            candidate.hash == hash
                && candidate.loader_ptr == loader_ptr
                && candidate
                    .value
                    .downcast_ref::<A::Owned>()
                    .is_some_and(|owned| args.cmp_owned(owned))
        };

        // Attempt to fetch an existing asset handle while holding a reader to the shared map.
        let assets = self.assets.read().unwrap();

        if let Some((_, asset)) = assets.raw_entry().from_hash(hash, check_candidate) {
            asset.deletion_candidate.store(false, Relaxed);
            let asset = asset.value.clone();
            drop(assets);
            return asset.downcast::<OnceLock<R>>().unwrap();
        }

        drop(assets);

        // Attempt to insert the asset into the map. Since we have to upgrade the lock in between
        // the check and the insertion, someone may have raced us to creating the entry.
        let mut assets = self.assets.write().unwrap();

        match assets.raw_entry_mut().from_hash(hash, check_candidate) {
            hash_map::RawEntryMut::Occupied(entry) => {
                let entry = entry.into_mut();
                *entry.deletion_candidate.get_mut() = false;
                let entry = entry.value.clone();
                drop(assets);

                entry.downcast::<OnceLock<R>>().unwrap()
            }
            hash_map::RawEntryMut::Vacant(entry) => {
                let value = Arc::new(OnceLock::<R>::new());
                entry.insert_with_hasher(
                    hash,
                    AssetKey {
                        hash,
                        loader_ptr,
                        // FIXME: Poisoning
                        value: smallbox!(args.to_owned()),
                    },
                    AssetValue {
                        deletion_candidate: AtomicBool::new(false),
                        value: value.clone(),
                    },
                    |entry| entry.hash,
                );
                value
            }
        }
    }
}

// === Asset === //

#[derive_where(Clone)]
pub struct Asset<T> {
    _asset: Arc<OnceLock<T>>,
    inner: NonNull<T>,
}

unsafe impl<T: Sync> Send for Asset<T> {}

unsafe impl<T: Sync> Sync for Asset<T> {}

impl<T: fmt::Debug> fmt::Debug for Asset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> Eq for Asset<T> {}

impl<T> PartialEq for Asset<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> Deref for Asset<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}
