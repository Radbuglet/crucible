use derive_where::derive_where;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::Peekable;

/// An iterator merging two properly sorted iterators into one properly sorted stream.
#[derive_where(Debug; Peekable<IL>: Debug, Peekable<IR>: Debug)]
#[derive_where(Clone; Peekable<IL>: Clone, Peekable<IR>: Clone)]
pub struct MergeSortedIter<IL: Iterator, IR: Iterator> {
	left: Peekable<IL>,
	right: Peekable<IR>,
}

impl<IL: Iterator, IR: Iterator> MergeSortedIter<IL, IR> {
	pub fn new(
		left: impl IntoIterator<IntoIter = IL>,
		right: impl IntoIterator<IntoIter = IR>,
	) -> Self {
		Self {
			left: left.into_iter().peekable(),
			right: right.into_iter().peekable(),
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
#[derive_where(Debug; IL: Debug, Peekable<IR>: Debug)]
#[derive_where(Clone; IL: Clone, Peekable<IR>: Clone)]
pub struct ExcludeSortedIter<IL: Iterator, IR: Iterator> {
	left: IL,
	right: Peekable<IR>,
}

impl<IL: Iterator, IR: Iterator> ExcludeSortedIter<IL, IR> {
	pub fn new(
		left: impl IntoIterator<IntoIter = IL>,
		right: impl IntoIterator<IntoIter = IR>,
	) -> Self {
		Self {
			left: left.into_iter(),
			right: right.into_iter().peekable(),
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
						// We don't remove `right` from the exclude iterator just yet because we might
						// find a duplicate.
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
