use std::{
	cell::Cell,
	future::{poll_fn, Future},
	pin::Pin,
	ptr::NonNull,
	task::{Context, Poll},
};

use derive_where::derive_where;
use dummy_waker::dummy_waker;

use crate::{
	dynamic_value,
	lang::iter::{ContextualIter, FlowResult},
};

use super::lifetime::DynamicRef;

// === ContinuationSig === //

mod sealed {
	pub trait CannotBeImplemented {}
}

pub trait ContinuationSig<'a>: sealed::CannotBeImplemented {
	type Input;
}

type Feed<'a, C> = <C as ContinuationSig<'a>>::Input;

pub type EmptyContinuator = dyn for<'a> ContinuationSig<'a, Input = ()>;

// === Yield === //

pub struct Yield<T, C: ?Sized + for<'a> ContinuationSig<'a> = EmptyContinuator> {
	state: Cell<YieldState<T, C>>,
}

#[derive_where(Default)]
enum YieldState<T, C: ?Sized + for<'a> ContinuationSig<'a>> {
	#[derive_where(default)]
	Meaningless,
	Polling,
	#[allow(clippy::type_complexity)]
	AwaitingContinuation(DynamicRef<dyn Fn(&mut Feed<'_, C>)>),
	ResolvedValue(T),
}

impl<T, C: ?Sized + for<'a> ContinuationSig<'a>> Default for Yield<T, C> {
	fn default() -> Self {
		Self {
			state: Cell::new(YieldState::Polling),
		}
	}
}

impl<T, C: ?Sized + for<'a> ContinuationSig<'a>> Yield<T, C> {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn ask<F, O>(&self, continuator: F) -> O
	where
		F: FnOnce(&'_ mut Feed<'_, C>) -> O,
	{
		// Construct the continuator function
		let continuator = &Cell::new(Some(continuator));
		let p_continuator = NonNull::from(continuator).cast::<()>();

		let output = &Cell::new(None);
		let p_output = NonNull::from(output).cast::<()>();

		dynamic_value! {
			let continuator: dyn Fn(&mut Feed<'_, C>) = move |input: &mut Feed<'_, C>| {
				unsafe {
					// Safety: `continuator` and `output` strictly outlive the `Dynamic` instance
					// owning this closure.
					let continuator = p_continuator.cast::<Cell<Option<F>>>().as_ref();
					let output = p_output.cast::<Cell<Option<O>>>().as_ref();

					output.set(Some((continuator.take().unwrap())(input)));
				};
			};
		}

		// Exchange `Polling` for `AwaitingContinuation`
		match self.state.take() {
			YieldState::Polling => {
				self.state
					.set(YieldState::AwaitingContinuation(continuator));
			}
			state => {
				self.state.set(state);
				panic!(
					"Cannot `ask` for a continuation while the generator caller is not polling."
				);
			}
		}

		// Wait until `AwaitingContinuation` gets replaced with `ResolvedContinuation`
		poll_fn(move |_| match self.state.take() {
			state @ YieldState::Polling => {
				self.state.set(state);
				Poll::Ready(output.take().unwrap())
			}
			state @ YieldState::AwaitingContinuation { .. } => {
				self.state.set(state);
				Poll::Pending
			}
			state => {
				self.state.set(state);
				panic!(
					"`AwaitingContinuation` state never resolved. Got into an unexpected state."
				);
			}
		})
		.await
	}

	pub async fn produce(&self, value: T) {
		// Exchange `Polling` for `ResolvedValue`
		match self.state.take() {
			YieldState::Polling => {
				self.state.set(YieldState::ResolvedValue(value));
			}
			state => {
				self.state.set(state);
				panic!("`Cannot `provide` a value while the generator caller is not polling.");
			}
		}

		// Wait until `AwaitingContinuation` gets replaced with `Polling`
		poll_fn(|_| match self.state.take() {
			state @ YieldState::Polling => {
				self.state.set(state);
				Poll::Ready(())
			}
			state @ YieldState::ResolvedValue(_) => {
				self.state.set(state);
				Poll::Pending
			}
			state => {
				self.state.set(state);
				panic!("`ResolvedValue` state never resolved. Got into an unexpected state.");
			}
		})
		.await
	}

	pub async fn produce_many<I: IntoIterator<Item = T>>(&self, values: I) {
		for value in values {
			self.produce(value).await;
		}
	}

	pub fn next(
		&self,
		mut future: Pin<&mut impl Future>,
		input: &mut <C as ContinuationSig<'_>>::Input,
	) -> FlowResult<T> {
		let waker = dummy_waker();
		let mut context = Context::from_waker(&waker);

		// Ensure that we're in the `Polling` state.
		match self.state.take() {
			state @ YieldState::Polling => {
				self.state.set(state);
			}
			state => {
				self.state.set(state);
				panic!("Cannot call `next` on a `Yield` instance in a non-neutral state.");
			}
		};

		// Continuously attempt to poll the iterator until we get a `ResolvedValue`.
		loop {
			match (future.as_mut().poll(&mut context), self.state.take()) {
				// Handle value resolutions
				(Poll::Ready(_), state @ YieldState::Polling) => {
					self.state.set(state);
					break FlowResult::Finished;
				}
				(Poll::Ready(_), YieldState::ResolvedValue(value)) => {
					self.state.set(YieldState::Polling);
					break FlowResult::FinishedWith(value);
				}
				(Poll::Pending, YieldState::ResolvedValue(value)) => {
					self.state.set(YieldState::Polling);
					break FlowResult::Proceed(value);
				}

				// Handle inert polling states
				(Poll::Pending, state @ YieldState::Polling) => {
					// We're still polling for updates. Continue the loop.
					self.state.set(state);
					continue;
				}

				// Handle continuations
				(Poll::Pending, YieldState::AwaitingContinuation(continuator)) => {
					// This is redundant but makes the logic clearer.
					self.state.set(YieldState::Meaningless);

					// Call the `continuator` on the input to get a continuation. If we panic,
					// we'll unwind the `continuator` before we unwind the actual dangerous future,
					// avoid an abort.
					(continuator)(input);

					self.state.set(YieldState::Polling);
				}

				// Handle illegal states
				(Poll::Ready(_), YieldState::AwaitingContinuation(continuator)) => {
					// This is redundant but makes the logic clearer.
					self.state.set(YieldState::Meaningless);

					// Ensure that we drop the `continuator` first. This is also redundant but
					// makes clear that we're always dropping the `continuator` before we
					// potentially drop the future.
					drop(continuator);

					panic!(
						"Generator is waiting for a continuator despite having finished polling."
					);
				}
				(_, YieldState::Meaningless) => {
					panic!("Encountered a meaningless state while polling. An internal error must have occurred.");
				}
			}
		}
	}

	pub fn iter<'a, F>(&'a self, future: Pin<&'a mut F>) -> YieldIterator<'_, F, T, C>
	where
		F: Future,
	{
		YieldIterator {
			yielder: self,
			future,
			is_done: false,
		}
	}
}

impl<T> Yield<T> {
	pub fn bind_empty_context(&self) {
		// (no-op)
	}
}

// === YieldIter === //

pub struct YieldIterator<'a, F, T, C: ?Sized + for<'b> ContinuationSig<'b>> {
	yielder: &'a Yield<T, C>,
	future: Pin<&'a mut F>,
	is_done: bool,
}

impl<'a, 'b, F, T, C> ContextualIter<Feed<'b, C>> for YieldIterator<'a, F, T, C>
where
	F: Future,
	C: ?Sized + for<'c> ContinuationSig<'c>,
{
	type Item = T;

	fn next_on_ref(&mut self, context: &mut Feed<'b, C>) -> Option<Self::Item> {
		if self.is_done {
			return None;
		}

		match self.yielder.next(self.future.as_mut(), context) {
			FlowResult::Proceed(value) => Some(value),
			FlowResult::FinishedWith(value) => {
				self.is_done = true;
				Some(value)
			}
			FlowResult::Finished => None,
		}
	}
}

#[doc(hidden)]
pub mod macro_internals {
	pub use {
		super::{ContinuationSig, Yield},
		core::pin::pin,
	};
}

#[macro_export]
macro_rules! use_generator {
	(let $name:ident[$yielder:ident] = $fn:expr) => {
		let y = &$crate::lang::generator::macro_internals::Yield::new();
		let future = $crate::lang::generator::macro_internals::pin!({
			let $yielder = y;
			$fn
		});
		let mut $name = y.iter(future);
	};
}

#[macro_export]
macro_rules! yielder {
	($out:ty$(; for<$lt:lifetime> $in:ty)?) => {
		$crate::lang::generator::macro_internals::Yield<
			$out
			$(,dyn for<$lt> ContinuationSig<$lt, Input = $in>,)?
		>
	};
}
