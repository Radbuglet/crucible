use memchr::memchr;
use std::ffi::CStr;
use std::os::raw::c_char;

// === FFI === //

pub const NUL_BYTE: u8 = 0;
pub const NUL_CHAR: c_char = NUL_BYTE as c_char;

/// Creates a [CStr] from a buffer of [c_char]s with an included nul character. The nul does not have
/// to be at the end of the buffer (and usually isn't because of the way Vulkan returns fixed sized
/// string buffers)
pub fn strbuf_to_cstr(chars: &[c_char]) -> &CStr {
	let chars = unsafe { &*(chars as *const _ as *const [u8]) };
	let nul_offset = memchr(NUL_BYTE, chars).expect("cstr must contain a nul byte!");
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
	strbuf_to_cstr(chars).to_str().unwrap()
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
	CStr::from_ptr(chars).to_str().unwrap()
}

/// Converts a nul-terminated [str] into a [CStr], panicking if the [str] is not nul-terminated.
/// Interior nul characters are not allowed.
pub fn str_to_cstr(str: &str) -> &CStr {
	CStr::from_bytes_with_nul(str.as_bytes()).unwrap()
}

/// Converts a [str] to a [*const c_char](c_char), but omits the check for a non-interior nul terminator
/// in release builds. Be careful with the lifetime of temporaries!
pub fn unchecked_str_to_strptr(str: &str) -> *const c_char {
	#[cfg(debug_assertions)]
	{
		str_to_cstr(str).as_ptr()
	}

	#[cfg(not(debug_assertions))]
	{
		str as *const str as *const c_char
	}
}

/// Checks whether two nul-terminated strptrs are equal.
pub unsafe fn strcmp(mut a_ptr: *const c_char, mut b_ptr: *const c_char) -> bool {
	// No SIMD because character arrays have no alignment guarantees/fixed lengths, causing potential
	// out of bounds memory accesses and UB.
	loop {
		let a = a_ptr.read();
		let b = b_ptr.read();

		// Detect buffer mismatch
		if a != b {
			return false;
		}

		// If one of the character is nul, we know that both strings have been terminated.
		if a == NUL_CHAR {
			return true;
		}

		a_ptr = a_ptr.add(1);
		b_ptr = b_ptr.add(1);
	}
}

pub macro static_cstr($str:expr) {
	str_to_cstr(concat!($str, "\0"))
}

pub macro static_strptr($str:expr) {
	unchecked_str_to_strptr(concat!($str, "\0"))
}

// === Formatting === //

// TODO: Somehow integrate with language's formatting machinery

pub fn format_list<'a, I>(list: I) -> String
where
	I: IntoIterator<Item = &'a str>,
{
	let mut builder = String::new();

	// Push elements
	for element in list {
		builder.push_str(element);
		builder.push_str(", ");
	}

	// Remove the trailing comma
	builder.pop();
	builder.pop();

	builder
}
