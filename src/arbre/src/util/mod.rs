mod any_value;
pub use any_value::*;

mod const_vec;
pub use const_vec::*;

mod perfect_map;
pub use perfect_map::*;

mod variance;
pub use variance::*;

pub fn ref_addr<T: ?Sized>(ptr: &T) -> *const () {
    (ptr as *const T).to_raw_parts().0
}
