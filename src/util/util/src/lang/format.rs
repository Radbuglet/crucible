use std::{cell::Cell, fmt};

pub fn display_from_fn(
	f: impl FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
) -> impl fmt::Display {
	struct Formatter<F>(Cell<Option<F>>);

	impl<F> fmt::Display for Formatter<F>
	where
		F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
	{
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			(self.0.take().unwrap())(f)
		}
	}

	Formatter(Cell::new(Some(f)))
}
