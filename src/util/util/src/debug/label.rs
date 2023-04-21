use std::{borrow::Cow, fmt};

pub type ReifiedDebugLabel = Option<Cow<'static, str>>;

pub trait DebugLabel: Sized {
	fn reify(self) -> ReifiedDebugLabel;

	fn reify_if_debug(self) -> ReifiedDebugLabel {
		if cfg!(debug_assertions) {
			self.reify()
		} else {
			None
		}
	}
}

pub const NO_LABEL: Option<&'static str> = None;

impl DebugLabel for String {
	fn reify(self) -> ReifiedDebugLabel {
		Some(Cow::Owned(self))
	}
}

impl DebugLabel for &'static str {
	fn reify(self) -> ReifiedDebugLabel {
		Some(Cow::Borrowed(self))
	}
}

impl DebugLabel for fmt::Arguments<'_> {
	fn reify(self) -> ReifiedDebugLabel {
		match self.as_str() {
			Some(static_str) => Some(Cow::Borrowed(static_str)),
			None => Some(Cow::Owned(format!("{self}"))),
		}
	}
}

impl<T: DebugLabel> DebugLabel for Option<T> {
	fn reify(self) -> ReifiedDebugLabel {
		self.and_then(DebugLabel::reify)
	}
}
