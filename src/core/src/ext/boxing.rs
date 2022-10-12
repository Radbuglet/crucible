pub fn leak_box<'a, T>(val: T) -> &'a mut T {
	Box::leak(Box::new(val))
}
