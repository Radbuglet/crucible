use std::{fmt, hash};

pub trait MetaEnum: 'static + Sized + fmt::Debug + Copy + hash::Hash + Eq {
    type Meta: Sized;

    fn variants() -> &'static [(Self, Self::Meta)];
    fn meta(&self) -> &'static Self::Meta {
        for (key, meta) in Self::variants().iter() {
            if self == key {
                return meta;
            }
        }
        unreachable!()
    }
}

pub macro meta_enum($(
    $vis:vis enum($meta_ty:ty) $ty_name:ident {
        $($var_name:ident = $meta_init:expr),*
        $(,)?
    }
)*) {$(
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    $vis enum $ty_name {
        $($var_name),*
    }

    impl $ty_name {
        const VAR_COUNT: usize = 0 $(+ {let _ = Self::$var_name; 1})*;
        const VARIANTS: [(Self, $meta_ty); Self::VAR_COUNT] = [$(
            (Self::$var_name, $meta_init),
        )*];
    }

    impl MetaEnum for $ty_name {
        type Meta = $meta_ty;

        fn variants() -> &'static [(Self, Self::Meta)] {
            &Self::VARIANTS
        }
    }
)*}
