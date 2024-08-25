#[macro_export]
macro_rules! gen_names {
    (
        $target:path [$($counter:tt)*] [$($banked_name:ident),*$(,)?] { $($regular:tt)* }
    ) => {
        $crate::gen_names! {
            @internal [] [$($banked_name)*]
            $target [$($counter)*] { $($regular)* }
        }
    };
    (
        @internal [$($accum:ident)*] [$first_remaining:ident $($next_remaining:ident)*]
        $target:path [$first_counter:tt $($counter:tt)*] { $($regular:tt)* }
    ) => {
        $crate::gen_names! {
            @internal [$($accum)* $first_remaining] [$($next_remaining)*]
            $target [$($counter)*] { $($regular)* }
        }
    };
    (
        @internal [$($accum:ident)*] [$($remaining:ident)*]
        $target:path [] { $($regular:tt)* }
    ) => {
        $target! { $($regular)* $($accum)* }
    };
}

pub use gen_names;
