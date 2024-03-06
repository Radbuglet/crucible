use std::time::Duration;

pub mod ffi;

fn main() {
    dbg!(ffi::get_api_version("foo.whee"));
    println!("Hello, world!");

    std::thread::sleep(Duration::from_millis(10000));

    ffi::set_shutdown_handler(42, |(data, msg): (&i32, String)| {
        dbg!(data, msg);
    });
}
