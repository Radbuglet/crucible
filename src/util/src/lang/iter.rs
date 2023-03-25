use std::{iter, ops::Deref};

use crate::mem::array::map_arr;

// === ContextualIter === //

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

// === VolumetricIter === //

#[derive(Debug, Clone)]
pub struct VolumetricIter<const N: usize> {
	pub pos: Option<[u32; N]>,
	pub max_inclusive: [u32; N],
}

impl<const N: usize> VolumetricIter<N> {
	pub fn new_exclusive(max: [u32; N]) -> Option<Self> {
		if max.iter().all(|&v| v > 0) {
			Some(Self::new_inclusive(map_arr(max, |v| v - 1)))
		} else {
			None
		}
	}

	pub fn new_exclusive_iter(max: [u32; N]) -> impl Iterator<Item = [u32; N]> {
		optionally_iter(Self::new_exclusive(max))
	}

	pub const fn new_inclusive(max: [u32; N]) -> Self {
		Self {
			pos: Some([0; N]),
			max_inclusive: max,
		}
	}

	pub fn next_capturing<F>(&mut self, mut on_rollback: F) -> Option<[u32; N]>
	where
		F: FnMut(usize),
	{
		// Handle the empty iterator special case.
		if N == 0 {
			return None;
		}

		// Save the previous result so our iterator includes (0, ..., 0) automatically.
		// If the `pos` is `None`, we have exhausted our iterator and can early-return.
		let pos = self.pos.as_mut()?;
		let next = *pos;

		// Update the position for the next query
		let mut i = N - 1;
		loop {
			// If we just yielded our maximum...
			if pos[i] >= self.max_inclusive[i] {
				// Wrap our value back to zero...
				pos[i] = 0;
				on_rollback(i);

				// And move on to update the next place value.
				if i > 0 {
					i -= 1;
				} else {
					// ...unless we've covered the entire volume.
					self.pos = None;
					break;
				}
			} else {
				pos[i] += 1;
				break;
			}
		}

		Some(next)
	}
}

impl<const N: usize> Iterator for VolumetricIter<N> {
	type Item = [u32; N];

	fn next(&mut self) -> Option<Self::Item> {
		self.next_capturing(|_| {})
	}
}

// === choice_iter === //

#[doc(hidden)]
pub mod macro_internals {
	pub use Iterator;
}

#[macro_export]
macro_rules! choice_iter {
	($vis:vis $name:ident : $($variant:ident),*$(,)?) => {
		$vis enum $name<$($variant),*> {
			$($variant($variant)),*
		}

		impl<__Item, $($variant),*> $crate::lang::iter::macro_internals::Iterator for $name<$($variant),*>
		where
			$($variant: $crate::lang::iter::macro_internals::Iterator<Item = __Item>),*
		{
			type Item = __Item;

			fn next(&mut self) -> Option<Self::Item> {
				match self {
					$(Self::$variant(v) => v.next()),*
				}
			}
		}
	};
}

// === optionally_iter === //

choice_iter!(pub Either : Left, Right);

pub fn optionally_iter<I: Iterator>(iter: Option<I>) -> impl Iterator<Item = I::Item> {
	match iter {
		Some(iter) => Either::Left(iter),
		None => Either::Right(iter::empty()),
	}
}

// === Smart Pointer Iteration === //

pub fn iter_wrapped_slice<'a, P, T>(ptr: P) -> impl Iterator<Item = T> + 'a
where
	P: Deref<Target = [T]> + 'a,
	T: Clone,
{
	let mut i = 0;

	iter::from_fn(move || {
		let v = ptr.get(i)?;
		i += 1;
		Some(v.clone())
	})
}
