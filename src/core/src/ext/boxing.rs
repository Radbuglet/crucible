pub fn leak_box<T>(value: T) -> &'static mut T {
	Box::leak(Box::new(value))
}
