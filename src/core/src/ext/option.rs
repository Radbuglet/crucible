use crate::std_traits::OptionLike;

pub trait OptionExt: OptionLike {
	fn is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool;
}

impl<T> OptionExt for Option<T> {
	fn is_some_and<F>(self, f: F) -> bool
	where
		F: FnOnce(Self::Value) -> bool,
	{
		self.map_or(false, f)
	}
}
