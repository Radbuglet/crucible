use derive_where::derive_where;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash, Hasher};
use std::mem::replace;

/// An iterator with one element of lookahead.
#[derive_where(Debug; I: Debug, I::Item: Debug)]
#[derive_where(Clone; I: Clone, I::Item: Clone)]
pub struct LookaheadIter<I: Iterator> {
	next: Option<I::Item>,
	iter: I,
}

impl<I: Iterator> LookaheadIter<I> {
	pub fn new(source: impl IntoIterator<IntoIter = I>) -> Self {
		let mut iter = source.into_iter();
		let next = iter.next();
		Self { next, iter }
	}

	pub fn peek(&self) -> Option<&I::Item> {
		self.next.as_ref()
	}
}

impl<I: Iterator> Iterator for LookaheadIter<I> {
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item> {
		replace(&mut self.next, self.iter.next())
	}
}

/// An iterator merging two properly sorted iterators into one properly sorted stream.
#[derive_where(Debug; LookaheadIter<IL>: Debug, LookaheadIter<IR>: Debug)]
#[derive_where(Clone; LookaheadIter<IL>: Clone, LookaheadIter<IR>: Clone)]
pub struct MergeSortedIter<IL: Iterator, IR: Iterator> {
	left: LookaheadIter<IL>,
	right: LookaheadIter<IR>,
}

impl<IL: Iterator, IR: Iterator> MergeSortedIter<IL, IR> {
	pub fn new(
		left: impl IntoIterator<IntoIter = IL>,
		right: impl IntoIterator<IntoIter = IR>,
	) -> Self {
		Self {
			left: LookaheadIter::new(left.into_iter()),
			right: LookaheadIter::new(right.into_iter()),
		}
	}
}

impl<T: Ord, IL: Iterator<Item = T>, IR: Iterator<Item = T>> Iterator for MergeSortedIter<IL, IR> {
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		match (self.left.peek(), self.right.peek()) {
			(Some(a), Some(b)) => match a.cmp(b) {
				Ordering::Less | Ordering::Equal => self.left.next(),
				Ordering::Greater => self.right.next(),
			},
			(Some(_), None) => self.left.next(),
			(None, Some(_)) => self.right.next(),
			(None, None) => None,
		}
	}
}

/// An iterator excluding elements in the properly sorted right stream from elements in the properly
/// sorted left stream.
#[derive_where(Debug; IL: Debug, LookaheadIter<IR>: Debug)]
#[derive_where(Clone; IL: Clone, LookaheadIter<IR>: Clone)]
pub struct ExcludeSortedIter<IL: Iterator, IR: Iterator> {
	left: IL,
	right: LookaheadIter<IR>,
}

impl<IL: Iterator, IR: Iterator> ExcludeSortedIter<IL, IR> {
	pub fn new(
		left: impl IntoIterator<IntoIter = IL>,
		right: impl IntoIterator<IntoIter = IR>,
	) -> Self {
		Self {
			left: left.into_iter(),
			right: LookaheadIter::new(right.into_iter()),
		}
	}
}

impl<T: Ord, IL: Iterator<Item = T>, IR: Iterator<Item = T>> Iterator
	for ExcludeSortedIter<IL, IR>
{
	type Item = T;

	fn next(&mut self) -> Option<Self::Item> {
		'left_scan: while let Some(next) = self.left.next() {
			// Check if the right list contains this element.
			'contain_chk: loop {
				let right = match self.right.peek() {
					Some(right) => right,
					// Nothing left to exclude; include this element.
					None => break,
				};

				match right.cmp(&next) {
					Ordering::Less => {
						// `right` is still less than `next` so a future element may still equal `next`.
						// Consume this element and continue checking.
						let _ = self.right.next();
					}
					Ordering::Equal => {
						// `right` is equal to `next` so we must exclude `next`.
						let _ = self.right.next();
						continue 'left_scan;
					}
					Ordering::Greater => {
						// `right` is greater than `next`. `next` is therefore included and this `right`
						// candidate remains in the list.
						break 'contain_chk;
					}
				}
			}

			// Return if not included.
			return Some(next);
		}

		None
	}
}

pub fn hash_iter<B, I>(builder: &B, iter: I) -> u64
where
	B: BuildHasher,
	I: IntoIterator,
	I::Item: Hash,
{
	let mut hasher = builder.build_hasher();
	for elem in iter {
		elem.hash(&mut hasher);
	}
	hasher.finish()
}

pub fn is_sorted<I>(list: I) -> bool
where
	I: IntoIterator,
	I::Item: Ord,
{
	let mut prev: Option<I::Item> = None;
	list.into_iter().all(move |val| {
		if let Some(prev) = prev.take() {
			if prev.cmp(&val) == Ordering::Greater {
				return false;
			}
		}

		prev = Some(val);
		true
	})
}
