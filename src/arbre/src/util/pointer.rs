use std::marker::Unsize;
use std::ptr::{Pointee, null};

pub const fn ref_addr<T: ?Sized>(ptr: &T) -> *const () {
    (ptr as *const T).to_raw_parts().0
}

pub const fn unsize_meta<A, B>() -> B::Metadata
where
    A: Unsize<B>,
    B: ?Sized + Pointee,
{
    let original = null::<A>();
    let transformed = original as *const B;
    transformed.to_raw_parts().1
}
