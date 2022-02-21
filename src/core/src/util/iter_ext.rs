use std::mem::MaybeUninit;

pub trait ArrayCollectExt: Sized + Iterator {
	fn try_collect_array<const N: usize>(mut self) -> Option<[<Self as Iterator>::Item; N]> {
		let mut arr = MaybeUninit::<Self::Item>::uninit_array::<N>();

		for slot in arr.iter_mut() {
			slot.write(match self.next() {
				Some(next) => next,
				None => return None,
			});
		}

		Some(unsafe { MaybeUninit::array_assume_init(arr) })
	}

	fn collect_array<const N: usize>(self) -> [<Self as Iterator>::Item; N] {
		match self.try_collect_array() {
			Some(array) => array,
			None => panic!(
				"Iterator must have at least {} element{}.",
				N,
				if N != 1 { "s" } else { "" }
			),
		}
	}

	fn collect_into(mut self, target: &mut [<Self as Iterator>::Item]) -> (usize, Self) {
		let mut filled = 0;
		while let Some(elem) = self.next() {
			if filled >= target.len() {
				return (filled, self);
			}
			target[filled] = elem;
			filled += 1;
		}
		(filled, self)
	}
}

impl<I: Iterator> ArrayCollectExt for I {}
