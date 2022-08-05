// When working with lifetime escape-hatches, we are better off being painfully explicit than
// being [explicit] painfully.
#[allow(clippy::needless_lifetimes)]
pub fn try_transform<'a, T: ?Sized, R: ?Sized, F>(
	orig: &'a mut T,
	f: F,
) -> Result<&'a mut R, &'a mut T>
where
	F: FnOnce(&mut T) -> Option<&mut R>,
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

pub fn try_transform_or_err<T, R, E, F>(
	orig: &mut T,
	f: F,
) -> Result<&mut R, (&mut T, E)>
where
	T: ?Sized,
	R: ?Sized,
	F: FnOnce(&mut T) -> Result<&mut R, E>,
{
	let mut err_reg = None;

	match try_transform(orig, |lent| match f(lent) {
		Ok(xformed) => Some(xformed),
		Err(err) => {
			err_reg = Some(err);
			None
		}
	}) {
		Ok(xformed) => Ok(xformed),
		Err(orig) => Err((orig, err_reg.unwrap())),
	}
}
