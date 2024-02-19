use crt_marshal::{generate_guest_ffi, WasmSlice, WasmStr};

generate_guest_ffi! {
    fn "crucible0".get_rt_mode() -> u32;

    fn "crucible0".test_payload(args: WasmSlice<WasmStr>);
}

fn main() {
    println!("Hello, world!");

    unsafe {
        test_payload(WasmSlice::new_guest(&[
            WasmStr::new_guest("whee"),
            WasmStr::new_guest("woo"),
        ]))
    };
}
