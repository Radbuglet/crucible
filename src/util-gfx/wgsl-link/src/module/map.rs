use std::{fmt, marker::PhantomData};

use derive_where::derive_where;

// === Core === //

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

#[derive_where(Copy, Clone, Default)]
pub struct MapNever<T>(PhantomData<fn() -> T>);

impl<T> MapNever<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Map<T, ()> for MapNever<T> {
    fn map(&self, _value: T) -> T {
        unimplemented!()
    }
}

#[derive_where(Copy, Clone, Default)]
pub struct MapIdentity<T>(PhantomData<fn() -> T>);

impl<T> MapIdentity<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Map<T, ()> for MapIdentity<T> {
    fn map(&self, value: T) -> T {
        value
    }
}

// Combinators
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

pub struct MapComplete<F, M>(pub F, pub M);

impl<T, F, M> Map<T, ()> for MapComplete<F, M>
where
    F: Fn(T, &M) -> T,
{
    fn map(&self, value: T) -> T {
        self.0(value, &self.1)
    }
}

pub struct MapAndComplete<F, M>(pub F, pub M);

impl<T, F, M> Map<T, (Disamb<0>, ())> for MapAndComplete<F, M>
where
    F: Fn(T, &M) -> T,
{
    fn map(&self, value: T) -> T {
        self.0(value, &self.1)
    }
}

impl<F, T, M, D> Map<T, (Disamb<1>, D)> for MapAndComplete<F, M>
where
    M: Map<T, D>,
{
    fn map(&self, value: T) -> T {
        self.1.map(value)
    }
}

pub struct MapUpcastMany<M, C: UpcastCollection>(pub M, pub PhantomData<fn() -> C>);

impl<T, D1, D2, M, C> Map<T, (D1, D2)> for MapUpcastMany<M, C>
where
    M: Map<T, D1>,
    C: UpcastCollectionHas<T, D2>,
{
    fn map(&self, value: T) -> T {
        self.0.map(value)
    }
}

pub trait MapCombinatorsExt: Sized {
    fn and<R>(self, other: R) -> MapCombine<Self, R> {
        MapCombine(self, other)
    }

    fn and_complete<F>(self, func: F) -> MapAndComplete<F, Self> {
        MapAndComplete(func, self)
    }

    fn complete<M>(self, other: M) -> MapComplete<Self, M> {
        MapComplete(self, other)
    }

    fn upcast_one<T, D>(&self) -> &impl Map<T, D>
    where
        Self: Map<T, D>,
    {
        self
    }

    fn upcast<C: UpcastCollection>(self, _collection: C) -> MapUpcastMany<Self, C> {
        MapUpcastMany(self, PhantomData)
    }
}

impl<T> MapCombinatorsExt for T {}

// `map_alias` macro
#[doc(hidden)]
pub mod map_alias_internals {
    pub use {super::Map, crucible_utils::macros::gen_names, Sized};
}

#[macro_export]
macro_rules! map_alias {
    ($(
        $vis:vis trait $name:ident $(<$($generic:ident),*$(,)?>)?
            $(: $([$($inherit_base:tt)*]$([$($inherit_arg:ty),*$(,)?])?),*$(,)?)?
            $( = $($ty:ty),*$(,)? )?;
    )*) => {$(
        $crate::module::map::map_alias_internals::gen_names! {
            $crate::module::map::map_alias [$($({ $ty })*)? $($({ $($inherit_base)* })*)?] [
                __Disamb1,
                __Disamb2,
                __Disamb3,
                __Disamb4,
                __Disamb5,
                __Disamb6,
                __Disamb7,
                __Disamb8,
                __Disamb9,
                __Disamb10,
                __Disamb11,
                __Disamb12,
            ] {
                @internal($name<Disambiguator, $($generic,)*>)
                $vis trait $name<$($($generic),*)?>:
                    $($( [$crate::module::map::map_alias_internals::Map][$ty], )*)?
                    $($( [$($inherit_base)*][$($($inherit_arg),*)?], )*)?;
            }
        }
    )*};
    (@internal($self_trait:ty)
        $vis:vis trait $name:ident <$($generic:ident),*>: $(
            [$($inherit_base:tt)*][$($inherit_arg:ty),*$(,)?]
        ),* $(,)?;

        $($disamb_name:ident)*
    ) => {
        $vis trait $name<
            Disambiguator,
            $($generic,)*
        >:
            $($($inherit_base)*<$($inherit_arg,)* <Self as $self_trait>::$disamb_name,> + )*
                $crate::module::map::map_alias_internals::Sized
        {
            $(type $disamb_name;)*
        }

        impl<
            $($disamb_name,)*
            $($generic,)*
            __T:
                $(
                    $($inherit_base)* <
                        $($inherit_arg,)*
                        $disamb_name,
                    >
                    +
                )*
                $crate::module::map::map_alias_internals::Sized,
        > $name<
            $($generic,)*
            ( $($disamb_name,)* ),
        > for __T {
            $(type $disamb_name = $disamb_name;)*
        }
    };
}

pub use map_alias;

// Upcast collection
pub trait UpcastCollection: Sized + fmt::Debug + Copy + Default {
    fn union<R: UpcastCollection>(self, other: R) -> UpcastCollectionAnd<Self, R> {
        UpcastCollectionAnd(self, other)
    }
}

pub trait UpcastCollectionHas<T, D>: UpcastCollection {}

#[derive(Debug, Copy, Clone, Default)]
pub struct UpcastCollectionEmpty;

impl UpcastCollection for UpcastCollectionEmpty {}

#[derive_where(Debug, Copy, Clone, Default)]
pub struct UpcastCollectionUnit<T>(pub PhantomData<fn() -> T>);

impl<T> UpcastCollectionUnit<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> UpcastCollection for UpcastCollectionUnit<T> {}

impl<T> UpcastCollectionHas<T, ()> for UpcastCollectionUnit<T> {}

#[derive(Debug, Copy, Clone, Default)]
pub struct UpcastCollectionAnd<L: UpcastCollection, R: UpcastCollection>(pub L, pub R);

impl<L: UpcastCollection, R: UpcastCollection> UpcastCollection for UpcastCollectionAnd<L, R> {}

impl<T, D, L, R> UpcastCollectionHas<T, (Disamb<0>, D)> for UpcastCollectionAnd<L, R>
where
    L: UpcastCollectionHas<T, D>,
    R: UpcastCollection,
{
}

impl<T, D, L, R: UpcastCollectionHas<T, D>> UpcastCollectionHas<T, (Disamb<1>, D)>
    for UpcastCollectionAnd<L, R>
where
    L: UpcastCollection,
    R: UpcastCollectionHas<T, D>,
{
}

// === Generic Mappers === //

pub fn map_collection<I, D>(v: I, f: &impl Map<I::Item, D>) -> I
where
    I: IntoIterator + FromIterator<I::Item>,
{
    v.into_iter().map(|v| f.map(v)).collect()
}

pub fn map_option<T, D>(v: Option<T>, f: &impl Map<T, D>) -> Option<T> {
    v.map(|v| f.map(v))
}
