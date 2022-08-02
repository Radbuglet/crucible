use sealed::sealed;

// === `WithContext` === //

#[derive(Debug, Clone)]
pub struct WithContext<C, I: ContextualIter<C>> {
	pub context: C,
	pub iter: I,
}

impl<C, I> Iterator for WithContext<C, I>
where
	I: ContextualIter<C>,
{
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next_on_ref(&mut self.context)
	}
}

pub trait ContextualIter<C>: Sized {
	type Item;

	fn next_on_ref(&mut self, context: &mut C) -> Option<Self::Item>;

	fn next(&mut self, mut context: C) -> Option<Self::Item> {
		self.next_on_ref(&mut context)
	}

	fn with_context(self, context: C) -> WithContext<C, Self> {
		WithContext {
			context,
			iter: self,
		}
	}
}

// === Flow Iter === //

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
