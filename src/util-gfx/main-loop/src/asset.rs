use std::{any::Any, borrow::Borrow, hash, sync::Arc};

use hash_utils::FxDashMap;
use newtypes::impl_tuples;

// === AssetManager === //

#[derive(Default)]
pub struct AssetManager {}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<C, A, R>(&self, cx: C, args: A, loader: fn(C, A) -> R) -> Asset<R>
    where
        A: for<'a> LoaderArgs<'a>,
        R: Any + Send + Sync,
    {
        todo!();
    }
}

// === Asset === //

pub struct Asset<T> {
    asset: Arc<T>,
}

// === LoaderArgs === //

pub trait LoaderArgs<'a>: Copy + hash::Hash + Eq {
    type Owned: Send + Sync;

    fn to_owned(self) -> Self::Owned;

    fn borrow_owned(owned: &'a Self::Owned) -> Self;
}

macro_rules! impl_loader_args {
    ($($name:ident:$field:tt),*) => {
        impl<'a, $($name: ToOwned + Eq + hash::Hash),*> LoaderArgs<'a> for ($(&'a $name,)*)
        where
            $($name::Owned: Send + Sync,)*
        {
            type Owned = ($($name::Owned,)*);

            #[allow(clippy::unused_unit)]
            fn to_owned(self) -> Self::Owned {
                ($(self.$field.to_owned(),)*)
            }

            #[allow(clippy::unused_unit)]
            #[allow(unused_variables)]
            fn borrow_owned(owned: &'a Self::Owned) -> Self {
                ($(owned.$field.borrow(),)*)
            }
        }

    };
}

impl_tuples!(impl_loader_args);
