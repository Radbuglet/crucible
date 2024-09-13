use std::iter;

use derive_where::derive_where;

#[derive_where(Clone; L, R, L::Item, R::Item)]
pub struct RemoveSortedIter<L: Iterator, R: Iterator> {
    pub source: iter::Peekable<L>,
    pub removed: iter::Peekable<R>,
}

impl<L: Iterator, R: Iterator> RemoveSortedIter<L, R> {
    pub fn new<T>(
        source: impl IntoIterator<IntoIter = L, Item = T>,
        removed: impl IntoIterator<IntoIter = R, Item = T>,
    ) -> Self {
        Self {
            source: source.into_iter().peekable(),
            removed: removed.into_iter().peekable(),
        }
    }
}

impl<T, L, R> Iterator for RemoveSortedIter<L, R>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
    T: Ord,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use std::cmp::Ordering::*;

        while let (Some(candidate), Some(removed)) = (self.source.peek(), self.removed.peek()) {
            match candidate.cmp(removed) {
                Less => {
                    self.removed.next();
                }
                Equal => {
                    self.source.next();
                }
                Greater => {
                    break;
                }
            }
        }

        self.source.next()
    }
}

#[derive_where(Clone; L, R, L::Item, R::Item)]
pub struct MergeSortedIter<L: Iterator, R: Iterator> {
    pub left: iter::Peekable<L>,
    pub right: iter::Peekable<R>,
}

impl<L: Iterator, R: Iterator> MergeSortedIter<L, R> {
    pub fn new<T>(
        left: impl IntoIterator<IntoIter = L, Item = T>,
        right: impl IntoIterator<IntoIter = R, Item = T>,
    ) -> Self {
        Self {
            left: left.into_iter().peekable(),
            right: right.into_iter().peekable(),
        }
    }
}

impl<T, L, R> Iterator for MergeSortedIter<L, R>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
    T: Ord,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.left.peek(), self.right.peek()) {
            (Some(l), Some(r)) => {
                if l < r {
                    self.left.next()
                } else {
                    self.right.next()
                }
            }
            (Some(_), None) => self.left.next(),
            (None, Some(_)) => self.right.next(),
            (None, None) => None,
        }
    }
}

#[derive_where(Clone; I, I::Item)]
pub struct DedupSortedIter<I: Iterator> {
    pub iter: iter::Peekable<I>,
}

impl<I: Iterator> DedupSortedIter<I> {
    pub fn new(iter: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            iter: iter.into_iter().peekable(),
        }
    }
}

impl<I: Iterator> Iterator for DedupSortedIter<I>
where
    I::Item: Ord,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.iter.next()?;

        while Some(&first) == self.iter.peek() {
            self.iter.next();
        }

        Some(first)
    }
}
