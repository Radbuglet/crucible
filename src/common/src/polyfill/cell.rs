use std::cell::{Ref, RefMut};

use bytemuck::TransparentWrapper;

use crate::polyfill::lifetime::try_transform;

pub fn filter_map_ref<'a, T, U, F>(orig: Ref<'a, T>, mut f: F) -> Result<Ref<'a, U>, Ref<'a, T>>
where
	F: FnMut(&T) -> Option<&U>,
{
	// Thanks to `kpreid` for the awesome insight behind this technique!
	let backup = Ref::clone(&orig);
	let mapped = Ref::map(orig, |orig| match f(orig) {
		Some(mapped) => std::slice::from_ref(mapped),
		None => &[],
	});

	if mapped.len() > 0 {
		Ok(Ref::map(mapped, |slice| &slice[0]))
	} else {
		Err(backup)
	}
}

pub fn filter_map_mut<'a, T, U, F>(
	orig: RefMut<'a, T>,
	f: F,
) -> Result<RefMut<'a, U>, RefMut<'a, T>>
where
	F: FnMut(&mut T) -> Option<&mut U>,
{
	// Utils
	// Thanks, again, to `kpreid` for helping me make the original implementation of this safe.
	trait Either<U, T> {
		fn as_result(&mut self) -> Result<&mut U, &mut T>;
	}

	#[derive(TransparentWrapper)]
	#[repr(transparent)]
	struct Success<U>(U);

	impl<U, T> Either<U, T> for Success<U> {
		fn as_result(&mut self) -> Result<&mut U, &mut T> {
			Ok(&mut self.0)
		}
	}

	#[derive(TransparentWrapper)]
	#[repr(transparent)]
	struct Failure<T>(T);

	impl<U, T> Either<U, T> for Failure<T> {
		fn as_result(&mut self) -> Result<&mut U, &mut T> {
			Err(&mut self.0)
		}
	}

	// Actual implementation
	let mut mapped = RefMut::map(orig, |orig| match try_transform(orig, f) {
		Ok(mapped) => {
			<Success<U> as TransparentWrapper<U>>::wrap_mut(mapped) as &mut dyn Either<U, T>
		}
		Err(orig) => <Failure<T> as TransparentWrapper<T>>::wrap_mut(orig) as &mut dyn Either<U, T>,
	});

	match mapped.as_result().is_ok() {
		true => Ok(RefMut::map(mapped, |val| val.as_result().ok().unwrap())),
		false => Err(RefMut::map(mapped, |val| val.as_result().err().unwrap())),
	}
}
