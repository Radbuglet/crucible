pub mod ffi;

fn main() {
    dbg!(ffi::get_api_version("foo.whee"));
    println!("Hello, world!");
}
