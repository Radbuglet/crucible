use std::{borrow::Borrow, hash};

use crate::newtypes::impl_tuples;

pub trait ManyToOwned: Copy + hash::Hash + Eq {
    type Owned;

    fn to_owned(self) -> Self::Owned;

    fn cmp_owned(&self, owned: &Self::Owned) -> bool;
}

macro_rules! impl_many_to_owned {
    ($($name:ident:$field:tt),*) => {
        impl<$($name: ToOwned + Eq + hash::Hash),*> ManyToOwned for ($(&'_ $name,)*)
        {
            type Owned = ($($name::Owned,)*);

            #[allow(clippy::unused_unit)]
            fn to_owned(self) -> Self::Owned {
                ($(self.$field.to_owned(),)*)
            }

            #[allow(clippy::unused_unit)]
            #[allow(unused_variables)]
            fn cmp_owned(&self, owned: &Self::Owned) -> bool {
                $(self.$field == owned.$field.borrow() &&)* true
            }
        }

    };
}

impl_tuples!(impl_many_to_owned);
