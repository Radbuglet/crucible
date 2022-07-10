pub fn try_transform<'a, T: ?Sized, R: ?Sized, F>(
	orig: &'a mut T,
	mut f: F,
) -> Result<&'a mut R, &'a mut T>
where
	F: FnMut(&mut T) -> Option<&mut R>,
{
	let orig_ptr = orig as *mut T;

	match f(orig) {
		Some(new) => Ok(new),
		None => Err(unsafe {
			// Safety: `f` can only hold onto `orig` for the duration of the function invocation.
			// Since nothing has been returned, we have exclusive access to `orig_ptr` and can acquire
			// a mutable reference to it as desired.
			&mut *orig_ptr
		}),
	}
}
