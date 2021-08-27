pub fn choose_first_bit(flag: u32) -> u32 {
	if flag > 0 {
		1 << flag.leading_zeros()
	} else {
		0
	}
}

pub macro choose_first_flag($ty:ty, $expr:expr) {{
	let bits: $ty = $expr;
	<$ty>::from_bits_truncate(choose_first_bit(bits.bits()))
}}
