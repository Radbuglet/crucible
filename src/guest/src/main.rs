use std::time::Duration;

pub mod ffi;

fn main() {
    dbg!(ffi::get_api_version("foo.whee"));
    println!("Hello, world!");

    std::thread::sleep(Duration::from_millis(100));

    let droopy = Droopy;

    ffi::set_shutdown_handler(move |data| {
        let _ = &droopy;
        println!("Shutdown handler called: {data:?}");
    });
}

struct Droopy;

impl Drop for Droopy {
    fn drop(&mut self) {
        println!("Dropped");
    }
}
