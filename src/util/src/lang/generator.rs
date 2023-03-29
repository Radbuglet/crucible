use core::{
	cell::Cell,
	future::{poll_fn, Future},
	iter,
	pin::Pin,
	task::{Context, Poll},
};

use dummy_waker::dummy_waker;

// === Core === //

pub struct Yield<T> {
	value: Cell<Option<T>>,
}

impl<T> Default for Yield<T> {
	fn default() -> Self {
		Self {
			value: Default::default(),
		}
	}
}

impl<T> Yield<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn produce(&self, value: T) {
		self.value.set(Some(value));

		poll_fn(|_| {
			let replaced = self.value.take();
			let is_none = replaced.is_none();
			self.value.set(replaced);

			if is_none {
				Poll::Ready(())
			} else {
				Poll::Pending
			}
		})
		.await;
	}

	pub async fn produce_many<I: IntoIterator<Item = T>>(&self, values: I) {
		for v in values {
			self.produce(v).await;
		}
	}

	pub fn take(&self) -> Option<T> {
		self.value.take()
	}

	pub fn iter<'a>(
		&'a self,
		mut future: Pin<&'a mut impl Future>,
	) -> impl Iterator<Item = T> + 'a {
		let mut is_done = false;
		iter::from_fn(move || {
			if is_done {
				return None;
			}

			loop {
				break match (
					future
						.as_mut()
						.poll(&mut Context::from_waker(&dummy_waker())),
					self.take(),
				) {
					(Poll::Pending, Some(value)) => Some(value),
					(Poll::Ready(_), Some(value)) => {
						is_done = true;
						Some(value)
					}
					(Poll::Pending, None) => continue,
					(Poll::Ready(_), None) => None,
				};
			}
		})
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
