#[macro_export]
macro_rules! gen_names {
    (
        $target:path [$($counter:tt)*] { $($regular:tt)* }
    ) => {
        $crate::gen_names! {
            @internal [] [
                __Macro1
                __Macro2
                __Macro3
                __Macro4
                __Macro5
                __Macro6
                __Macro7
                __Macro8
                __Macro9
                __Macro10
                __Macro11
                __Macro12
            ]
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
