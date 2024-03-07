use std::alloc::Layout;

use crt_marshal::{
    generate_guest_export, guest_u32_to_usize, WasmFunc, WasmPtr, WasmSlice, WasmStr, ZstFn,
};

// === Allocator Entry === //

generate_guest_export! {
    fn pre_init() {
        // TODO: Initialize logger
    }

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
    crt_marshal::generate_guest_import! {
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
    crt_marshal::generate_guest_import! {
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

pub fn set_shutdown_handler<T, F>(args: T, handler: F)
where
    T: 'static + Send + Sync,
    F: for<'a> ZstFn<(&'a T, String), Output = ()>,
{
    crt_marshal::generate_guest_import! {
        fn "crucible0_lifecycle".set_shutdown_handler(
            args: WasmPtr<()>,
            handler: WasmFunc<(WasmPtr<()>, WasmStr)>,
        );
    }

    unsafe {
        let _ = handler;
        let value = Box::into_raw(Box::new(args));

        set_shutdown_handler(
            WasmPtr::new_guest(value.cast()),
            WasmFunc::new_guest(|(args, data): (WasmPtr<()>, WasmStr)| {
                F::call_static((&*args.into_guest().cast::<T>(), data.into_guest_string()))
            }),
        )
    }
}

pub fn write_reload_message(data: &[u8]) {
    crt_marshal::generate_guest_import! {
        fn "crucible0_lifecycle".write_reload_message(slice: WasmSlice<u8>);
    }

    unsafe { write_reload_message(WasmSlice::new_guest(data)) };
}

pub fn read_reload_message(buf: &mut [u8]) -> usize {
    crt_marshal::generate_guest_import! {
        fn "crucible0_lifecycle".write_reload_message(buf: WasmSlice<u8>) -> u32;
    }

    guest_u32_to_usize(unsafe { write_reload_message(WasmSlice::new_guest(buf)) })
}

pub fn clear_reload_message() {
    crt_marshal::generate_guest_import! {
        fn "crucible0_lifecycle".clear_reload_message();
    }

    unsafe { clear_reload_message() };
}

// === Peers === //
