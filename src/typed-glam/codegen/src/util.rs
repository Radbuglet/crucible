use genco::prelude::*;

pub trait FmtIntoIterExt<L: Lang>: Sized + IntoIterator {
	fn to_token_fmt(self) -> FmtIntoIter<Self::IntoIter> {
		FmtIntoIter {
			iter: self.into_iter(),
		}
	}
}

impl<L, E, T> FmtIntoIterExt<L> for T
where
	L: Lang,
	E: FormatInto<L>,
	T: IntoIterator<Item = E>,
{
}

pub struct FmtIntoIter<I> {
	iter: I,
}

impl<L, E, I> FormatInto<L> for FmtIntoIter<I>
where
	L: Lang,
	E: FormatInto<L>,
	I: Iterator<Item = E>,
{
	fn format_into(self, tokens: &mut Tokens<L>) {
		for elem in self.iter {
			elem.format_into(tokens);
		}
	}
}
