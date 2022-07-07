use genco::prelude::*;

pub trait FmtIterExt: Sized + IntoIterator {
	fn fmt_delimited<D>(self, delimiter: D) -> DelimitedTokenIterFmt<Self::IntoIter, D> {
		DelimitedTokenIterFmt {
			iter: self.into_iter(),
			delimiter,
		}
	}
}

impl<T: IntoIterator> FmtIterExt for T {}

#[derive(Debug, Clone)]
pub struct DelimitedTokenIterFmt<I, D> {
	iter: I,
	delimiter: D,
}

impl<L, I, E, D> FormatInto<L> for DelimitedTokenIterFmt<I, D>
where
	L: Lang,
	I: Iterator<Item = E>,
	E: FormatInto<L>,
	D: Delimiter<L>,
{
	fn format_into(mut self, tokens: &mut Tokens<L>) {
		if let Some(first) = self.iter.next() {
			first.format_into(tokens);
		} else {
			return;
		}

		for remaining in self.iter {
			self.delimiter.write_delimiter(tokens);
			remaining.format_into(tokens);
		}
	}
}

pub trait Delimiter<L: Lang> {
	fn write_delimiter(&mut self, target: &mut Tokens<L>);
}

impl<L: Lang, T: FormatInto<L> + Clone> Delimiter<L> for T {
	fn write_delimiter(&mut self, target: &mut Tokens<L>) {
		self.clone().format_into(target)
	}
}

// Note: This type must not be `Clone` to avoid conflicts with the `impl` block above.
pub struct DelimiterFn<F>(pub F);

impl<L, F, O> Delimiter<L> for DelimiterFn<F>
where
	L: Lang,
	F: FnMut() -> O,
	O: FormatInto<L>,
{
	fn write_delimiter(&mut self, target: &mut Tokens<L>) {
		(self.0)().format_into(target)
	}
}

pub struct NoOpDelimiter;

impl<L: Lang> Delimiter<L> for NoOpDelimiter {
	fn write_delimiter(&mut self, _target: &mut Tokens<L>) {}
}

pub trait FmtIntoOwnedExt<L: Lang> {
	fn fmt_to_tokens(self) -> Tokens<L>;
}

impl<L: Lang, T: FormatInto<L>> FmtIntoOwnedExt<L> for T {
	fn fmt_to_tokens(self) -> Tokens<L> {
		let mut target = Tokens::new();
		self.format_into(&mut target);
		target
	}
}
