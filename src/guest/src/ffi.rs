use std::alloc::Layout;

use crt_marshal::{
    guest_export, guest_u32_to_usize, WasmDynamicFunc, WasmFunc, WasmPtr, WasmSlice, WasmStr,
};

// === Allocator Entry === //

guest_export! {
    #[no_mangle]
    fn pre_init() {
        // TODO: Initialize logger
    }

    #[no_mangle]
    fn host_alloc(size: u32, align: u32) -> WasmPtr<u8> {
        unsafe {
            let layout = Layout::from_size_align(guest_u32_to_usize(size), guest_u32_to_usize(align))
                .expect("invalid layout");

            let ptr = std::alloc::alloc(layout);

            WasmPtr::new_guest(ptr)
        }
    }
}

// === Version === //

pub fn get_api_version(namespace: &'static str) -> Option<semver::Version> {
    crt_marshal::guest_import! {
        fn "crucible0_version".get_api_version(namespace: WasmStr) -> WasmStr;
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
    DedicatedServer = 0,
    DedicatedClient = 1,
    IntegratedServer = 2,
    IntegratedClient = 3,
    SinglePlayerClient = 4,
    SinglePlayerServer = 5,
    ServerMod = 6,
    ClientMod = 7,
}

pub fn get_rt_mode() -> RtMode {
    crt_marshal::guest_import! {
        fn "crucible0_version".get_rt_mode() -> u8;
    }

    match unsafe { get_rt_mode() } {
        0 => RtMode::DedicatedServer,
        1 => RtMode::DedicatedClient,
        2 => RtMode::IntegratedServer,
        3 => RtMode::IntegratedClient,
        4 => RtMode::SinglePlayerClient,
        5 => RtMode::SinglePlayerServer,
        6 => RtMode::ServerMod,
        7 => RtMode::ClientMod,
        m => unreachable!("unknown runtime mode {m}"),
    }
}

// === Reloads === //

pub fn set_shutdown_handler(handler: impl 'static + Send + Sync + Fn(&str)) {
    crt_marshal::guest_import! {
        fn "crucible0_lifecycle".set_shutdown_handler(
            handler: WasmDynamicFunc<(WasmStr,)>,
        );
    }

    unsafe {
        set_shutdown_handler(WasmDynamicFunc::new_guest(Box::new(
            move |(str,): (WasmStr,)| handler(str.into_guest_string().as_str()),
        )))
    }
}

pub fn write_reload_message(data: &[u8]) {
    crt_marshal::guest_import! {
        fn "crucible0_lifecycle".write_reload_message(slice: WasmSlice<u8>);
    }

    unsafe { write_reload_message(WasmSlice::new_guest(data)) };
}

pub fn read_reload_message(buf: &mut [u8]) -> usize {
    crt_marshal::guest_import! {
        fn "crucible0_lifecycle".write_reload_message(buf: WasmSlice<u8>) -> u32;
    }

    guest_u32_to_usize(unsafe { write_reload_message(WasmSlice::new_guest(buf)) })
}

pub fn clear_reload_message() {
    crt_marshal::guest_import! {
        fn "crucible0_lifecycle".clear_reload_message();
    }

    unsafe { clear_reload_message() };
}

// === Peers === //
