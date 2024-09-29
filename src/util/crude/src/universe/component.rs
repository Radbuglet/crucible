use std::{any::TypeId, fmt, sync::OnceLock};

use crucible_utils::{impl_tuples, mem::Splicer};
use dashmap::DashMap;
use derive_where::derive_where;

use crate::Storage;

// === Component === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ComponentId(TypeId);

impl ComponentId {
    pub fn of<T: Component>() -> Self {
        Self(TypeId::of::<T>())
    }
}

pub trait Component: 'static {
    type Storage: Storage<Component = Self>;
}

// === Obj === //

pub type StorageOf<T> = <T as Component>::Storage;
pub type RawObj<T> = <StorageOf<T> as Storage>::Handle;

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Obj<T: Component>(RawObj<T>);

impl<T: Component> Obj<T> {
    pub fn from_raw(handle: RawObj<T>) -> Self {
        Self(handle)
    }

    pub fn raw(&self) -> RawObj<T> {
        self.0
    }
}

// === Bundles === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ErasedBundle(fn(&mut Vec<ComponentId>));

impl fmt::Debug for ErasedBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReifiedBundle").finish_non_exhaustive()
    }
}

impl ErasedBundle {
    pub fn of<B: Bundle>() -> Self {
        Self(B::write_component_list)
    }

    pub fn normalized(self) -> &'static [ComponentId] {
        static CACHE: OnceLock<DashMap<ErasedBundle, &'static [ComponentId]>> = OnceLock::new();

        let cache = CACHE.get_or_init(Default::default);

        if let Some(cached) = cache.get(&self) {
            return *cached;
        }

        *cache.entry(self).or_insert_with(|| {
            let mut components = Vec::new();
            self.0(&mut components);
            components.sort();

            let mut splicer = Splicer::new(&mut components);
            loop {
                let remaining = splicer.remaining();
                let Some((first_dup_idx, _)) = remaining
                    .windows(2)
                    .enumerate()
                    .find(|(_, win)| win[0] == win[1])
                else {
                    break;
                };

                let first_dup = remaining[first_dup_idx];
                let after_dup = &remaining[first_dup_idx..][1..];
                let dup_seq_len = after_dup
                    .iter()
                    .enumerate()
                    .find(|(_, &other)| first_dup != other)
                    .map_or(after_dup.len(), |v| v.0);

                splicer.splice(first_dup_idx, dup_seq_len, &[]);
            }
            drop(splicer);

            Box::leak(components.into_boxed_slice())
        })
    }
}

pub trait Bundle {
    fn write_component_list(target: &mut Vec<ComponentId>);
}

impl<T: Component> Bundle for T {
    fn write_component_list(target: &mut Vec<ComponentId>) {
        target.push(ComponentId::of::<T>())
    }
}

macro_rules! impl_bundle {
    ($($para:ident:$field:tt),*) => {
        impl<$($para: Bundle),*> Bundle for ($($para,)*) {
            #[allow(unused_variables)]
            fn write_component_list(target: &mut Vec<ComponentId>) {
                $($para::write_component_list(target);)*
            }
        }
    };
}

impl_tuples!(impl_bundle);
