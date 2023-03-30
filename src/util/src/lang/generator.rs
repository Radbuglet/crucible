use core::{
	cell::Cell,
	future::{poll_fn, Future},
	pin::Pin,
	task::{Context, Poll},
};

use derive_where::derive_where;
use dummy_waker::dummy_waker;

use crate::lang::iter::ContextualIter;

// === Core === //

pub trait Continuator {
	type Reified: Default;
	type Input<'a>;
	type Output;

	fn reify(self) -> Self::Reified;
	fn produce_value(reified: &mut Self::Reified, input: &mut Self::Input<'_>) -> Self::Output;
}

#[derive(Default)]
pub struct EmptyContinuator;

impl Continuator for EmptyContinuator {
	type Reified = Self;
	type Input<'a> = ();
	type Output = ();

	fn reify(self) -> Self::Reified {
		self
	}

	fn produce_value(_reified: &mut Self::Reified, (): &mut Self::Input<'_>) -> Self::Output {
		()
	}
}

#[derive_where(Default)]
pub struct Yield<T, C: Continuator = EmptyContinuator> {
	continuator: Cell<C::Reified>,
	state: Cell<YieldState<C::Output, T>>,
}

#[derive_where(Default)]
enum YieldState<I, O> {
	#[derive_where(default)]
	Waiting,
	Continue(I),
	Output(O),
}

impl<T> Yield<T> {
	pub fn set_empty_continuator(&self) {
		self.set_continuator(EmptyContinuator);
	}
}

impl<T, C: Continuator> Yield<T, C> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn set_continuator(&self, continuator: C) {
		self.continuator.set(continuator.reify());
	}

	pub async fn produce(&self, value: T) -> C::Output {
		self.state.set(YieldState::Output(value));

		poll_fn(|_| match self.state.take() {
			YieldState::Continue(value) => Poll::Ready(value),
			taken @ _ => {
				self.state.set(taken);
				Poll::Pending
			}
		})
		.await
	}

	pub async fn produce_many<I: IntoIterator<Item = T>>(&self, values: I) {
		for v in values {
			self.produce(v).await;
		}
	}

	pub fn read(&self) -> Option<T> {
		match self.state.take() {
			YieldState::Output(value) => Some(value),
			state @ _ => {
				self.state.set(state);
				None
			}
		}
	}

	pub fn write(&self, input: &mut C::Input<'_>) {
		let mut continuator = self.continuator.take();
		let value = C::produce_value(&mut continuator, input);
		self.continuator.set(continuator);
		self.state.set(YieldState::Continue(value));
	}

	pub fn contextual_iter<'a, F>(&'a self, future: Pin<&'a mut F>) -> YieldIterator<'a, F, T, C>
	where
		F: Future,
	{
		YieldIterator {
			yielder: self,
			future,
			is_done: false,
		}
	}

	pub fn iter_contextual<'a>(
		&'a self,
		future: Pin<&'a mut impl Future>,
		input: C::Input<'a>,
	) -> impl Iterator<Item = T> + 'a {
		self.contextual_iter(future).with_context(input)
	}

	pub fn iter<'a>(&'a self, future: Pin<&'a mut impl Future>) -> impl Iterator<Item = T> + 'a
	where
		C::Input<'a>: Default,
	{
		self.iter_contextual(future, Default::default())
	}
}

pub struct YieldIterator<'a, F, T, C: Continuator> {
	yielder: &'a Yield<T, C>,
	future: Pin<&'a mut F>,
	is_done: bool,
}

impl<'a, 'b, F, T, C> ContextualIter<C::Input<'b>> for YieldIterator<'a, F, T, C>
where
	F: Future,
	C: Continuator,
{
	type Item = T;

	fn next_on_ref(&mut self, context: &mut C::Input<'b>) -> Option<Self::Item> {
		// Ensure that we're not polling a completed future.
		if self.is_done {
			return None;
		}

		// Otherwise, provide a continuation value
		self.yielder.write(context);

		// And poll the future until we get a result
		loop {
			break match (
				self.future
					.as_mut()
					.poll(&mut Context::from_waker(&dummy_waker())),
				self.yielder.read(),
			) {
				// If the future is on-going and we have a value, yield it.
				(Poll::Pending, Some(value)) => Some(value),

				// If the future is finished and we have a value, yield the value but mark
				// the iterator as done.
				(Poll::Ready(_), Some(value)) => {
					self.is_done = true;
					Some(value)
				}

				// If the future is ongoing but we don't have a value, keep on polling the
				// future to force progress.
				(Poll::Pending, None) => continue,

				// If the future is finished and we don't have a value, the future has yielded
				// its last value and we can be finished.
				(Poll::Ready(_), None) => None,
			};
		}
	}
}

#[doc(hidden)]
pub mod macro_internals {
	pub use {super::Yield, core::pin::pin};
}

#[macro_export]
macro_rules! use_generator {
	(let $name:ident[$yielder:ident] = $fn:expr) => {
		let y = &$crate::lang::generator::Yield::new();
		let future = pin!({
			let $yielder = y;
			$fn
		});
		let $name = y.iter(future);
	};
}
