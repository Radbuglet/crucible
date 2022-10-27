use std::{borrow::Cow, fmt};

pub type ReifiedDebugLabel = Option<Cow<'static, str>>;

pub trait DebugLabel {
	fn reify(self) -> ReifiedDebugLabel;
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
		if let Some(static_str) = self.as_str() {
			Some(Cow::Borrowed(static_str))
		} else {
			Some(Cow::Owned(format!("{self}")))
		}
	}
}

impl<T: DebugLabel> DebugLabel for Option<T> {
	fn reify(self) -> ReifiedDebugLabel {
		self.and_then(DebugLabel::reify)
	}
}
