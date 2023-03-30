use std::{
	cell::Cell,
	future::{poll_fn, Future},
	marker::PhantomPinned,
	pin::pin,
	pin::Pin,
	process::abort,
	task::{Context, Poll},
};

use derive_where::derive_where;
use dummy_waker::dummy_waker;

use crate::lang::iter::{ContextualIter, FlowResult};

// === ContinuationSig === //

mod sealed {
	pub trait CannotBeImplemented {}
}

pub trait ContinuationSig: sealed::CannotBeImplemented {
	type Output;
}

pub trait ContinuationSigIn<'a>: ContinuationSig {
	type Input;
}

type FeedIn<'a, C> = <C as ContinuationSigIn<'a>>::Input;

pub type EmptyContinuator = dyn for<'a> ContinuationSigIn<'a, Input = (), Output = ()>;

// === LoanedFunc === //

// LoanedFunc
#[repr(C)]
pub struct LoanedFunc<F> {
	// Although the function may, itself, support unpinning, we want to ensure that this
	// structure remains in place so we can rely on references to `func` being pinned.
	_pinned: PhantomPinned,
	is_referenced: bool,
	func: F,
}

impl<F> LoanedFunc<F> {
	pub fn new(func: F) -> Self {
		Self {
			_pinned: PhantomPinned,
			is_referenced: false,
			func,
		}
	}
}

impl<F> Drop for LoanedFunc<F> {
	fn drop(&mut self) {
		if self.is_referenced {
			abort();
		}
	}
}

// LoanedFuncRef
pub struct LoanedFuncRef<C>
where
	C: ?Sized + for<'a> ContinuationSigIn<'a>,
{
	// Function Pointer Safety: for this function to be safe to call, the caller must assert that
	// the first pointer has a type appropriate for closure being executed (enforced by the
	// structure's invariants) and that the pointer remains alive for the duration of the
	// invocation.
	handler: unsafe fn(*mut (), &mut FeedIn<'_, C>) -> C::Output,
	data: *mut (),
}

impl<C> LoanedFuncRef<C>
where
	C: ?Sized + for<'a> ContinuationSigIn<'a>,
{
	pub fn new<F>(func: Pin<&mut LoanedFunc<F>>) -> Self
	where
		F: FnMut(&mut FeedIn<'_, C>) -> C::Output,
	{
		let func = unsafe {
			// Safety: TODO
			func.get_unchecked_mut()
		};

		// Construct a trampoline to execute our loaned function
		unsafe fn handler<C, F>(data: *mut (), input: &mut FeedIn<'_, C>) -> C::Output
		where
			C: ?Sized + for<'a> ContinuationSigIn<'a>,
			F: FnMut(&mut FeedIn<'_, C>) -> C::Output,
		{
			// Safety: provided by caller
			let data = &mut *(data as *mut LoanedFunc<F>);
			(data.func)(input)
		}

		// Mark ourselves as the referencer of this function.
		assert!(!func.is_referenced);
		func.is_referenced = true;

		// Construct a loan
		Self {
			handler: handler::<C, F>,
			data: func as *mut LoanedFunc<F> as *mut (),
		}
	}

	pub fn call(&mut self, input: &mut FeedIn<'_, C>) -> C::Output {
		unsafe {
			// Safety: the function pointer contract requires that we prove that `self.data` has
			// the appropriate type and that the pointee will remain valid (i.e. not dropped or
			// invalidated) for the duration of the invocation.
			//
			// We already know that `self.data` has the appropriate type because this structure
			// enforces that as an invariant.
			//
			// We know that `self.data` must be valid until the end of this function call because:
			// a) it was `Pin`'ned, guaranteeing that its memory will not be invalidated until the
			//    `LoanedFunc` destructor is called and
			// b) if the `LoanedFunc` destructor is called and the `is_referenced` flag is still
			//    set—a flag which will only get unset when we get `Drop`'ed—the process will
			//    abort, preventing the memory from ever being invalidated.
			(self.handler)(self.data, input)
		}
	}
}

impl<C> Drop for LoanedFuncRef<C>
where
	C: ?Sized + for<'a> ContinuationSigIn<'a>,
{
	fn drop(&mut self) {
		unsafe {
			// Safety: TODO
			(*(self.data as *mut LoanedFunc<()>)).is_referenced = false;
		}
	}
}

// === Yield === //

pub struct Yield<T, C: ?Sized + for<'a> ContinuationSigIn<'a> = EmptyContinuator> {
	state: Cell<YieldState<T, C>>,
}

#[derive_where(Default)]
enum YieldState<T, C: ?Sized + for<'a> ContinuationSigIn<'a>> {
	#[derive_where(default)]
	Meaningless,
	Polling,
	AwaitingContinuation(LoanedFuncRef<C>),
	ResolvedContinuation(C::Output),
	ResolvedValue(T),
}

impl<T, C: ?Sized + for<'a> ContinuationSigIn<'a>> Default for Yield<T, C> {
	fn default() -> Self {
		Self {
			state: Cell::new(YieldState::Polling),
		}
	}
}

impl<T, C: ?Sized + for<'a> ContinuationSigIn<'a>> Yield<T, C> {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn ask<F>(&self, continuator: F) -> C::Output
	where
		F: FnOnce(&'_ mut FeedIn<'_, C>) -> C::Output,
	{
		// Construct the continuator function
		let mut continuator = Some(continuator);
		let continuator = pin!(LoanedFunc::new(move |input: &'_ mut FeedIn<'_, C>| {
			(continuator.take().unwrap())(input)
		}));

		// Exchange `Polling` for `AwaitingContinuation`
		match self.state.take() {
			YieldState::Polling => {
				self.state
					.set(YieldState::AwaitingContinuation(LoanedFuncRef::new(
						continuator,
					)));
			}
			state @ _ => {
				self.state.set(state);
				panic!(
					"Cannot `ask` for a continuation while the generator caller is not polling."
				);
			}
		}

		// Wait until `AwaitingContinuation` gets replaced with `ResolvedContinuation`
		poll_fn(|_| match self.state.take() {
			YieldState::ResolvedContinuation(value) => {
				self.state.set(YieldState::Polling);
				Poll::Ready(value)
			}
			state @ YieldState::AwaitingContinuation { .. } => {
				self.state.set(state);
				Poll::Pending
			}
			state @ _ => {
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
			state @ _ => {
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
			state @ _ => {
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
		input: &mut <C as ContinuationSigIn<'_>>::Input,
	) -> FlowResult<T> {
		let waker = dummy_waker();
		let mut context = Context::from_waker(&waker);

		// Ensure that we're in the `Polling` state.
		match self.state.take() {
			state @ YieldState::Polling => {
				self.state.set(state);
			}
			state @ _ => {
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
				(
					Poll::Pending,
					state @ YieldState::Polling | state @ YieldState::ResolvedContinuation(_),
				) => {
					// We're still polling for updates. Continue the loop.
					self.state.set(state);
					continue;
				}

				// Handle continuations
				(Poll::Pending, YieldState::AwaitingContinuation(mut continuator)) => {
					// This is redundant but makes the logic clearer.
					self.state.set(YieldState::Meaningless);

					// Call the `continuator` on the input to get a continuation. If we panic,
					// we'll unwind the `continuator` before we unwind the actual dangerous future,
					// avoid an abort.
					let resolved = continuator.call(input);
					self.state.set(YieldState::ResolvedContinuation(resolved));
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
				(Poll::Ready(_), YieldState::ResolvedContinuation(_)) => {
					panic!("Provided a continuation value that was somehow never consumed by the generator.");
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

pub struct YieldIterator<'a, F, T, C: ?Sized + for<'b> ContinuationSigIn<'b>> {
	yielder: &'a Yield<T, C>,
	future: Pin<&'a mut F>,
	is_done: bool,
}

impl<'a, 'b, F, T, C> ContextualIter<FeedIn<'b, C>> for YieldIterator<'a, F, T, C>
where
	F: Future,
	C: ?Sized + for<'c> ContinuationSigIn<'c>,
{
	type Item = T;

	fn next_on_ref(&mut self, context: &mut FeedIn<'b, C>) -> Option<Self::Item> {
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
	pub use {super::Yield, core::pin::pin};
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
