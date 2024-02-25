use std::alloc::Layout;

use crt_marshal::WasmStr;

// === Allocator Entry === //

#[no_mangle]
unsafe extern "C" fn host_alloc(size: usize, align: usize) -> *mut u8 {
    std::alloc::alloc(Layout::from_size_align(size, align).expect("invalid layout"))
}

// === Version === //

pub fn get_api_version(namespace: &'static str) -> Option<semver::Version> {
    crt_marshal::generate_guest_ffi! {
        pub fn "crucible0".get_api_version(namespace: WasmStr) -> WasmStr;
    }

    unsafe {
        let version = get_api_version(WasmStr::new_guest(namespace));

        if !version.into_guest().is_null() {
            Some(
                semver::Version::parse(&version.into_guest_string())
                    .expect("failed to parse version"),
            )
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RtMode {
    DedicatedServer,
    DedicatedClient,
    IntegratedServer,
    IntegratedClient,
    ClientMod,
}

pub fn get_rt_mode() -> RtMode {
    todo!();
}

// === Reloads === //

pub fn write_reload_message(data: &[u8]) {
    todo!();
}

pub fn read_reload_message(buf: &mut [u8]) -> usize {
    todo!();
}

pub fn clear_reload_message() {
    todo!();
}

// === Peers === //
