pub fn unwrap_either<T>(result: Result<T, T>) -> T {
	match result {
		Ok(val) => val,
		Err(val) => val,
	}
}

pub fn swap_parts<T, E>(result: Result<T, E>) -> Result<E, T> {
	match result {
		Ok(ok) => Err(ok),
		Err(err) => Ok(err),
	}
}
