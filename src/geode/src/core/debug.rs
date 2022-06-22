use std::{borrow::Cow, fmt};

pub trait DebugLabel {
	fn to_debug_label(self) -> Option<Cow<'static, str>>;
}

#[derive(Debug, Copy, Clone)]
pub struct NoLabel;

impl DebugLabel for NoLabel {
	fn to_debug_label(self) -> Option<Cow<'static, str>> {
		None
	}
}

impl DebugLabel for String {
	fn to_debug_label(self) -> Option<Cow<'static, str>> {
		Some(Cow::Owned(self))
	}
}

impl DebugLabel for &'static str {
	fn to_debug_label(self) -> Option<Cow<'static, str>> {
		Some(Cow::Borrowed(self))
	}
}

impl DebugLabel for fmt::Arguments<'_> {
	fn to_debug_label(self) -> Option<Cow<'static, str>> {
		if let Some(static_str) = self.as_str() {
			Some(Cow::Borrowed(static_str))
		} else {
			Some(Cow::Owned(format!("{self}")))
		}
	}
}

impl<T: DebugLabel> DebugLabel for Option<T> {
	fn to_debug_label(self) -> Option<Cow<'static, str>> {
		self.and_then(DebugLabel::to_debug_label)
	}
}
