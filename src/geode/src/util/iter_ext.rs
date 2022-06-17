use std::cell::Cell;
use std::fmt::{Debug, Formatter};

pub struct DebugListIter<I>(Cell<Option<I>>);

impl<I> DebugListIter<I> {
	pub fn new(iter: I) -> Self {
		Self(Cell::new(Some(iter)))
	}
}

impl<I: IntoIterator<Item = T>, T: Debug> Debug for DebugListIter<I> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut builder = f.debug_list();
		let iter = self
			.0
			.replace(None)
			.expect("`DebugListIter` can only be displayed once.");

		for item in iter {
			builder.entry(&item);
		}
		builder.finish()
	}
}
