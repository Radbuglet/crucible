pub trait OptionLike: Sized {
	type Value;

	fn raw_option(self) -> Option<Self::Value>;
}

impl<T> OptionLike for Option<T> {
	type Value = T;

	fn raw_option(self) -> Option<Self::Value> {
		self
	}
}

impl<T, E> OptionLike for Result<T, E> {
	type Value = T;

	fn raw_option(self) -> Option<Self::Value> {
		self.ok()
	}
}

pub trait ResultLike: Sized {
	type Success;
	type Error;

	fn raw_result(self) -> Result<Self::Success, Self::Error>;
}

impl<T, E> ResultLike for Result<T, E> {
	type Success = T;
	type Error = E;

	fn raw_result(self) -> Result<Self::Success, Self::Error> {
		self
	}
}
