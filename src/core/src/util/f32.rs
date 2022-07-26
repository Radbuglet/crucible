pub fn frac_to_f32(num: u32, max: u32) -> Option<f32> {
	if max != 0 {
		// Yes, there are truncation errors with this routine. However, none of the routines
		// using this object are dealing with big fractions so this is fine.
		Some((num as f64 / max as f64) as f32)
	} else {
		None
	}
}
