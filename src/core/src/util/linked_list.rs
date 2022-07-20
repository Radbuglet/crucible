use crate::contextual_iter::{ContextualIter, WithContext};

// TODO: Strongly type non-sentinel nodes so we can achieve better performance.
pub trait LinkedList<N: Copy + Eq> {
	fn sentinel(&self) -> N;
	fn is_sentinel(&self, node: N) -> bool;

	fn head(&self) -> N {
		self.get_next(self.sentinel())
	}

	fn tail(&self) -> N {
		self.get_prev(self.sentinel())
	}

	fn get_pair(&self, node: N) -> (N, N) {
		(self.get_prev(node), self.get_next(node))
	}

	fn get_prev(&self, node: N) -> N;
	fn get_next(&self, node: N) -> N;

	fn set_pair(&mut self, node: N, prev: N, next: N) {
		self.set_prev(node, prev);
		self.set_next(node, next);
	}

	fn set_prev(&mut self, node: N, val: N);
	fn set_next(&mut self, node: N, val: N);

	fn replace_pair(&mut self, node: N, prev: N, next: N) -> (N, N) {
		(self.replace_prev(node, prev), self.replace_next(node, next))
	}

	fn replace_prev(&mut self, node: N, val: N) -> N {
		let old = self.get_prev(node);
		self.set_prev(node, val);
		old
	}

	fn replace_next(&mut self, node: N, val: N) -> N {
		let old = self.get_next(node);
		self.set_next(node, val);
		old
	}

	fn bond(&mut self, prev: N, next: N) {
		self.set_next(prev, next);
		self.set_prev(next, prev);
	}

	fn bond_trio(&mut self, prev: N, middle: N, next: N) {
		self.set_next(prev, middle);
		self.set_pair(middle, prev, next);
		self.set_prev(next, middle);
	}

	fn bond_replace(&mut self, prev: N, next: N) -> (N, N) {
		let prev_next = self.replace_next(prev, next);
		let next_prev = self.replace_next(next, prev);
		(prev_next, next_prev)
	}

	fn unlink(&mut self, node: N) {
		let (prev, next) = self.get_pair(node);
		self.bond(prev, next);
	}

	fn insert_before(&mut self, node: N, next: N) {
		self.unlink(node);

		// New layout:
		// [prev] [node] [next]
		// (4 connections)
		self.bond_trio(self.get_prev(next), node, next);
	}

	fn insert_after(&mut self, node: N, prev: N) {
		self.unlink(node);

		// New layout:
		// [prev] [node] [next]
		// (4 connections)
		self.bond_trio(prev, node, self.get_next(prev));
	}

	fn insert_head(&mut self, node: N) {
		self.insert_after(node, self.sentinel());
	}

	fn insert_tail(&mut self, node: N) {
		self.insert_before(node, self.sentinel());
	}

	fn iter_forwards(&self) -> WithContext<&'_ Self, ListIterForwards<N>> {
		self.iter_forwards_interactive().with_context(self)
	}

	fn iter_forwards_interactive(&self) -> ListIterForwards<N> {
		ListIterForwards {
			next_yielded: self.head(),
			end_at: self.sentinel(),
		}
	}

	fn iter_backwards(&self) -> WithContext<&'_ Self, ListIterBackwards<N>> {
		self.iter_backwards_interactive().with_context(self)
	}

	fn iter_backwards_interactive(&self) -> ListIterBackwards<N> {
		ListIterBackwards {
			next_yielded: self.tail(),
			end_at: self.sentinel(),
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ListIterForwards<N> {
	pub next_yielded: N,
	pub end_at: N,
}

impl<'a, L, N> ContextualIter<&'a L> for ListIterForwards<N>
where
	L: ?Sized + LinkedList<N>,
	N: Copy + Eq,
{
	type Item = N;

	fn next_on_ref(&mut self, list: &mut &'a L) -> Option<Self::Item> {
		let node = self.next_yielded;

		if node == self.end_at {
			return None;
		}

		self.next_yielded = list.get_next(self.next_yielded);

		Some(node)
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ListIterBackwards<N> {
	pub next_yielded: N,
	pub end_at: N,
}

impl<'a, L, N> ContextualIter<&'a L> for ListIterBackwards<N>
where
	L: ?Sized + LinkedList<N>,
	N: Copy + Eq,
{
	type Item = N;

	fn next_on_ref(&mut self, list: &mut &'a L) -> Option<Self::Item> {
		let node = self.next_yielded;

		if node == self.end_at {
			return None;
		}

		self.next_yielded = list.get_prev(self.next_yielded);

		Some(node)
	}
}
