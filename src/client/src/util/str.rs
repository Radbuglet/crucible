use super::error::ResultExt;
use memchr::memchr;
use std::ffi::CStr;
use std::os::raw::c_char;

/// Creates a [CStr] from a buffer of `c_chars` with an included nul character. The nul does not have
/// to be at the end of the buffer (and usually isn't because of the way Vulkan returns fixed sized
/// string buffers)
pub fn strbuf_to_cstr(chars: &[c_char]) -> &CStr {
    let chars = unsafe { &*(chars as *const _ as *const [u8]) };
    let nul_offset = memchr(0, chars).expect("cstr must contain a nul byte!");
    let chars = &chars[0..=nul_offset];

    unsafe { CStr::from_bytes_with_nul_unchecked(chars) }
}

/// Creates a [str] from a buffer of `c_chars` with an included nul character. The nul does not have
/// to be at the end of the buffer (and usually isn't because of the way Vulkan returns fixed sized
/// string buffers)
pub fn strbuf_to_str(chars: &[c_char]) -> &str {
    strbuf_to_cstr(chars).to_str().unwrap_pretty()
}

pub unsafe fn strptr_to_str<'a>(chars: *const c_char) -> &'a str {
    CStr::from_ptr(chars).to_str().unwrap_pretty()
}

/// Converts a nul-terminated [str] into a [CStr].
pub fn str_to_cstr(str: &str) -> &CStr {
    CStr::from_bytes_with_nul(str.as_bytes()).unwrap_pretty()
}

/// Converts a nul-terminated [str] into a `*const c_char`.
pub fn str_to_strbuf(str: &str) -> *const c_char {
    str_to_cstr(str).as_ptr()
}
