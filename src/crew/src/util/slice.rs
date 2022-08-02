pub fn limit_len<T>(slice: &[T], max_len: usize) -> &[T] {
	if slice.len() > max_len {
		&slice[..max_len]
	} else {
		slice
	}
}
