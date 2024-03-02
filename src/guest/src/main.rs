pub mod ffi;

fn main() {
    dbg!(ffi::get_api_version("foo.whee"));
    println!("Hello, world!");

    ffi::set_reload_handler(42, |(data, msg): (&i32, String)| {
        dbg!(data, msg);
    });
}
