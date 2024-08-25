// === Map === //

mod sealed {
    #[non_exhaustive]
    pub struct Disamb<const N: usize>;
}

use sealed::*;

// Traits
pub trait Map<T, D>: Sized {
    fn map(&self, value: T) -> T;
}

impl<M, T, D> Map<T, D> for &'_ M
where
    M: Map<T, D>,
{
    fn map(&self, value: T) -> T {
        (*self).map(value)
    }
}

pub struct MapFn<F>(pub F);

impl<T, F: Fn(T) -> T> Map<T, ()> for MapFn<F> {
    fn map(&self, value: T) -> T {
        self.0(value)
    }
}

// Combine
pub struct MapCombine<L, R>(pub L, pub R);

impl<T, D, C1: Map<T, D>, C2> Map<T, (Disamb<0>, D)> for MapCombine<C1, C2> {
    fn map(&self, value: T) -> T {
        self.0.map(value)
    }
}

impl<T, D, C1, C2: Map<T, D>> Map<T, (Disamb<1>, D)> for MapCombine<C1, C2> {
    fn map(&self, value: T) -> T {
        self.1.map(value)
    }
}

pub trait MapCombineExt: Sized {
    fn and<R>(self, other: R) -> MapCombine<Self, R> {
        MapCombine(self, other)
    }
}

impl<T> MapCombineExt for T {}

// === Mapper === //

#[doc(hidden)]
pub mod map_alias_internals {
    pub use {crucible_utils::macros::gen_names, Sized};
}

#[macro_export]
macro_rules! map_alias {
    ($(
        $vis:vis trait $name:ident $(<$($generic:ident),*$(,)?>)? = $($ty:ty),*$(,)?;
    )*) => {$(
        $crate::module::map::map_alias_internals::gen_names! {
            $crate::module::map::map_alias [$({ $ty })*] {
                @internal
                $vis trait $name<$($($generic),*)?> = $($ty),*;
            }
        }
    )*};
    (@internal
        $vis:vis trait $name:ident <$($generic:ident),*> = $($ty:ty),*;
        $($disamb_name:ident)*
    ) => {
        $vis trait $name<Disambiguator, $($generic,)*>: $($crate::module::map::Map<$ty, Self::$disamb_name>+)* $crate::module::map::map_alias_internals::Sized {
            $(type $disamb_name;)*
        }

        impl<$($disamb_name,)* $($generic,)* __T: $($crate::module::map::Map<$ty, $disamb_name>+)* $crate::module::map::map_alias_internals::Sized> $name<$($generic,)* ($($disamb_name,)*),> for __T {
            $(type $disamb_name = $disamb_name;)*
        }
    };
}

pub use map_alias;
