use super::error::ResultExt;
use memchr::memchr;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::os::raw::c_char;

/// Creates a [CStr] from a buffer of [c_char]s with an included nul character. The nul does not have
/// to be at the end of the buffer (and usually isn't because of the way Vulkan returns fixed sized
/// string buffers)
pub fn strbuf_to_cstr(chars: &[c_char]) -> &CStr {
    let chars = unsafe { &*(chars as *const _ as *const [u8]) };
    let nul_offset = memchr(0, chars).expect("cstr must contain a nul byte!");
    let chars = &chars[0..=nul_offset];

    unsafe {
        // Safety: We've already checked for the nul character and ensured that it was the last character
        // in the new sub-slice.
        CStr::from_bytes_with_nul_unchecked(chars)
    }
}

/// Creates a [str] from a buffer of [c_char]s with an included nul character. The nul does not have
/// to be at the end of the buffer (and usually isn't because of the way Vulkan returns fixed sized
/// string buffers)
pub fn strbuf_to_str(chars: &[c_char]) -> &str {
    strbuf_to_cstr(chars).to_str().unwrap_pretty()
}

/// Converts a raw [c_char] pointer array to a [str], panicking if the conversion from [CStr] to [str]
/// fails.
///
/// ## Safety
///
/// See [CStr::from_ptr]'s safety section.
///
pub unsafe fn strptr_to_str<'a>(chars: *const c_char) -> &'a str {
    // Safety: provided by caller
    CStr::from_ptr(chars).to_str().unwrap_pretty()
}

/// Converts a nul-terminated [str] into a [CStr], panicking if the [str] is not nul-terminated.
/// Interior nul characters are not allowed.
pub fn str_to_cstr(str: &str) -> &CStr {
    CStr::from_bytes_with_nul(str.as_bytes()).unwrap_pretty()
}

/// A raw [CStr] pointer with lifetime information. This object, unlike its raw [*const c_char](c_char)
/// counterpart, has the additional benefit of extending the lifetime of the temporary [CStr] passed
/// to it.
///
/// See also: [unwrap_raw_cstr_slice]
#[repr(transparent)] // TODO: Is this system actually necessary?
pub struct RawCStr<'a> {
    _lifetime: PhantomData<fn(&'a ())>,
    ptr: *const c_char,
}

impl<'a> RawCStr<'a> {
    pub unsafe fn wrap_ptr(ptr: *const c_char) -> Self {
        Self {
            _lifetime: PhantomData,
            ptr,
        }
    }

    pub fn from_str(str_: &'a str) -> Self {
        Self::from_cstr(str_to_cstr(str_))
    }

    pub fn from_cstr(str_: &'a CStr) -> Self {
        unsafe { Self::wrap_ptr(str_.as_ptr()) }
    }

    pub fn as_cstr(&self) -> &'a CStr {
        unsafe {
            // Safety: `ptr` comes from a `CStr` that is still alive.
            CStr::from_ptr::<'a>(self.ptr)
        }
    }
}

pub fn unwrap_raw_cstr_slice<'a, 'b>(slice: &'a [RawCStr<'b>]) -> &'a [*const c_char] {
    // TODO: Generalize as part of safe transmute system (we could also wait for the safe_transmute WG)
    unsafe {
        // Safety: `RawCStr` is `repr(transparent)` for `*const c_char`.
        std::mem::transmute::<&'a [RawCStr<'b>], &'a [*const c_char]>(slice)
    }
}

pub use erupt::cstr as static_cstr;
