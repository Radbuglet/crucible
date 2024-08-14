use std::mem;

pub fn smuggle_drop<T, V>(value: T, f: impl FnOnce(&T) -> &V) -> V {
    let sub_field = f(&value) as *const V;
    let value_start = &value as *const T as usize;
    assert!((value_start..(value_start + mem::size_of::<T>())).contains(&(sub_field as usize)));
    let sub_field = unsafe { sub_field.read() };
    mem::forget(value);
    sub_field
}
