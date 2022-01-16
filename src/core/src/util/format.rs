use std::fmt::{Display, Formatter};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FormatMs(pub Duration);

impl Display for FormatMs {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{0:.2}ms", self.0.as_secs_f64() * 1000.)
	}
}
