#[must_use]
pub fn round_up_u64(value: u64, align: u64) -> u64 {
	assert!(align.is_power_of_two());
	let mask = align - 1;

	(value.saturating_add(mask)) & !mask
}
