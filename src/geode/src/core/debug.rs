use std::{borrow::Cow, fmt};

pub type SerializedDebugLabel = Option<Cow<'static, str>>;

pub trait DebugLabel {
	fn to_debug_label(self) -> SerializedDebugLabel;
}

#[derive(Debug, Copy, Clone)]
pub struct NoLabel;

impl DebugLabel for NoLabel {
	fn to_debug_label(self) -> SerializedDebugLabel {
		None
	}
}

impl DebugLabel for String {
	fn to_debug_label(self) -> SerializedDebugLabel {
		Some(Cow::Owned(self))
	}
}

impl DebugLabel for &'static str {
	fn to_debug_label(self) -> SerializedDebugLabel {
		Some(Cow::Borrowed(self))
	}
}

impl DebugLabel for fmt::Arguments<'_> {
	fn to_debug_label(self) -> SerializedDebugLabel {
		if let Some(static_str) = self.as_str() {
			Some(Cow::Borrowed(static_str))
		} else {
			Some(Cow::Owned(format!("{self}")))
		}
	}
}

impl<T: DebugLabel> DebugLabel for Option<T> {
	fn to_debug_label(self) -> SerializedDebugLabel {
		self.and_then(DebugLabel::to_debug_label)
	}
}
