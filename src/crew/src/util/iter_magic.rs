use sealed::sealed;

pub fn limit_len<T>(slice: &[T], max_len: usize) -> &[T] {
	if slice.len() > max_len {
		&slice[..max_len]
	} else {
		slice
	}
}

#[sealed]
pub trait FlowIterExt: Sized + Iterator {
	fn flow(self) -> FlowIter<Self>;
}

#[sealed]
impl<T, I: Iterator<Item = Flow<T>>> FlowIterExt for I {
	fn flow(self) -> FlowIter<Self> {
		FlowIter {
			iter: self,
			stopped: false,
		}
	}
}

#[derive(Debug, Clone)]
pub struct FlowIter<I> {
	iter: I,
	stopped: bool,
}

impl<T, I: Iterator<Item = Flow<T>>> Iterator for FlowIter<I> {
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		if self.stopped {
			return None;
		}

		match self.iter.next() {
			Some(Flow::Yield(value)) => Some(value),
			Some(Flow::Break(value)) => {
				self.stopped = true;
				Some(value)
			}
			None => {
				self.stopped = true;
				None
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Flow<T> {
	Yield(T),
	Break(T),
}
