use core::borrow::Borrow;
use core::fmt::{Debug, Formatter};
use core::ops::{Bound, RangeBounds};

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct AnyRange<T> {
	pub start: Bound<T>,
	pub end: Bound<T>,
}

impl<T: Debug> Debug for AnyRange<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match &self.start {
			Bound::Included(start) => Debug::fmt(start, f)?,
			Bound::Excluded(start) => {
				Debug::fmt(start, f)?;
				write!(f, "!")?;
			}
			Bound::Unbounded => {}
		}

		f.write_str("..")?;

		match &self.start {
			Bound::Included(start) => {
				write!(f, "=")?;
				Debug::fmt(start, f)?;
			}
			Bound::Excluded(start) => Debug::fmt(start, f)?,
			Bound::Unbounded => {}
		}

		Ok(())
	}
}

impl<T: Clone> AnyRange<T> {
	// We can't use `From::from` because it conflicts with the identity conversion.
	pub fn new<R: Borrow<RI>, RI: RangeBounds<T>>(range: R) -> Self {
		let range = range.borrow();
		Self {
			start: range.start_bound().cloned(),
			end: range.end_bound().cloned(),
		}
	}
}

impl<T> RangeBounds<T> for AnyRange<T> {
	fn start_bound(&self) -> Bound<&T> {
		bound_as_ref(&self.start)
	}

	fn end_bound(&self) -> Bound<&T> {
		bound_as_ref(&self.end)
	}
}

pub fn bound_as_ref<T>(bound: &Bound<T>) -> Bound<&T> {
	match bound {
		Bound::Included(bound) => Bound::Included(bound),
		Bound::Excluded(bound) => Bound::Excluded(bound),
		Bound::Unbounded => Bound::Unbounded,
	}
}

pub fn unwrap_or_unbounded<T>(bound: Bound<T>) -> Option<T> {
	match bound {
		Bound::Included(val) => Some(val),
		Bound::Excluded(val) => Some(val),
		Bound::Unbounded => None,
	}
}
