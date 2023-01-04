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

// === Volumetric === //

#[derive(Debug, Clone)]
pub struct VolumetricIter<const N: usize> {
	pos: Option<[u32; N]>,
	max: [u32; N],
}

impl<const N: usize> VolumetricIter<N> {
	pub const fn new(max: [u32; N]) -> Self {
		Self {
			pos: Some([0; N]),
			max,
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
		let next = pos.clone();

		// Update the position for the next query
		let mut i = N - 1;
		loop {
			// If we're at our maximum...
			if pos[i] >= self.max[i] {
				// Wrap our value back to zero...
				pos[i] = 0;
				on_rollback(i);

				// And move on to update the next place value.
				if i > 0 {
					i -= 1;
				} else {
					// ...unless we've the entire volume.
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
