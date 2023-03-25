#[doc(hidden)]
pub mod macro_internals {
	use std::{
		cell::Cell,
		future::Future,
		marker::PhantomData,
		pin::Pin,
		ptr,
		task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
	};

	pub mod re_export {
		pub use core::{future::Future, ops::FnOnce};
	}

	fn dummy_waker() -> Waker {
		const VTABLE: RawWakerVTable = RawWakerVTable::new(
			|data: *const ()| RawWaker::new(data, &VTABLE),
			|_data: *const ()| (),
			|_data: *const ()| (),
			|_data: *const ()| (),
		);

		const RAW: RawWaker = RawWaker::new(ptr::null(), &VTABLE);

		unsafe { Waker::from_raw(RAW) }
	}

	thread_local! {
		static POLL_OUTPUT_PTR: Cell<*mut ()> = const { Cell::new(ptr::null_mut()) };
	}

	pub trait SelfRefOutput<'a>: 'static {
		type Type: Sized;
	}

	pub struct SelfRef<T: ?Sized, F> {
		_ty: PhantomData<fn(T) -> T>,
		future: F,
	}

	impl<T, F> SelfRef<T, F>
	where
		T: ?Sized + for<'a> SelfRefOutput<'a>,
		F: Future,
	{
		pub fn new_actually_very_unsafe<G>(generator: G) -> Self
		where
			G: FnOnce(SelfRefProvider<T>) -> F,
		{
			Self {
				_ty: PhantomData,
				future: generator(SelfRefProvider { _ty: PhantomData }),
			}
		}

		pub fn get(self: Pin<&mut Self>) -> <T as SelfRefOutput<'_>>::Type {
			// First, we project the pin to the future.
			let future = unsafe { self.map_unchecked_mut(|me| &mut me.future) };

			// Then, we allocate a slot for our output and set the register to point to it.
			let mut output = None::<<T as SelfRefOutput<'_>>::Type>;

			POLL_OUTPUT_PTR.with(|ptr| {
				ptr.set(&mut output as *mut Option<<T as SelfRefOutput<'_>>::Type> as *mut ());
			});

			// Next, we poll from our future to write to our output
			let Poll::Pending = future.poll(&mut Context::from_waker(&dummy_waker())) else {
				panic!("SelfRef expression caused an unexpected early return.");
			};

			// Finally, we read from our output.
			output.expect("SelfRef expression never produced a value.")
		}
	}

	pub struct SelfRefProvider<T: ?Sized> {
		_ty: PhantomData<fn(T) -> T>,
	}

	impl<T: ?Sized + for<'a> SelfRefOutput<'a>> SelfRefProvider<T> {
		pub async fn provide_actually_very_unsafe<'a>(
			&self,
			value: <T as SelfRefOutput<'a>>::Type,
		) {
			// Write to the output register
			POLL_OUTPUT_PTR.with(|ptr| unsafe {
				*ptr.get().cast::<Option<<T as SelfRefOutput<'a>>::Type>>() = Some(value);
			});

			// If this future ever gets polled again, panic.
			struct WakeUpAfterPoll(bool);

			impl Future for WakeUpAfterPoll {
				type Output = ();

				fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
					let me = self.get_mut();

					if me.0 {
						Poll::Ready(())
					} else {
						me.0 = true;
						Poll::Pending
					}
				}
			}

			WakeUpAfterPoll(false).await;

			panic!("`SelfRef::get` can only be called once.");
		}
	}
}

#[macro_export]
macro_rules! self_ref {
    ($ty:ty ; $future_lifetime:lifetime) => {
		$crate::macro_internals::SelfRef<
			dyn for<'a> $crate::macro_internals::SelfRefOutput<'a, Type = $ty>,
			impl $crate::macro_internals::re_export::Future<Output = ()> + $future_lifetime,
		>
	};
	($($setup_expr:tt)*) => {
		$crate::macro_internals::SelfRef::new_actually_very_unsafe(|__provider| async move {
			__provider.provide_actually_very_unsafe({ $($setup_expr)* }).await;
		})
	};
}
