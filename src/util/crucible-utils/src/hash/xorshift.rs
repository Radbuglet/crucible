use std::num::NonZeroU64;

pub const fn xorshift64_raw(state: u64) -> u64 {
    // Adapted from: https://en.wikipedia.org/w/index.php?title=Xorshift&oldid=1123949358
    let state = state ^ (state << 13);
    let state = state ^ (state >> 7);
    let state = state ^ (state << 17);
    state
}

pub const fn xorshift64(state: NonZeroU64) -> NonZeroU64 {
    unsafe { NonZeroU64::new_unchecked(xorshift64_raw(state.get())) }
}
