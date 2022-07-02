pub fn unwrap_either<T>(result: Result<T, T>) -> T {
	match result {
		Ok(val) => val,
		Err(val) => val,
	}
}
