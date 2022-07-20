use std::cell::Cell;

use crucible_core::linked_list::LinkedList;
use geode::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct ObjLinkedList<'s, N: 's + Copy, F> {
	pub session: Session<'s>,
	pub head: &'s Cell<Option<N>>,
	pub tail: &'s Cell<Option<N>>,
	pub access: F,
}

impl<'s, N, F> ObjLinkedList<'s, N, F>
where
	N: 's + Copy,
	F: Fn(Session<'s>, N) -> (&'s Cell<Option<N>>, &'s Cell<Option<N>>),
{
	pub fn get_prev_cell(&self, node: Option<N>) -> &'s Cell<Option<N>> {
		match node {
			Some(val) => &(self.access)(self.session, val).0,
			None => self.tail,
		}
	}

	pub fn get_next_cell(&self, node: Option<N>) -> &'s Cell<Option<N>> {
		match node {
			Some(val) => &(self.access)(self.session, val).1,
			None => self.head,
		}
	}
}

impl<'s, N, F> LinkedList<Option<N>> for ObjLinkedList<'s, N, F>
where
	N: 's + Copy + Eq,
	F: Fn(Session<'s>, N) -> (&'s Cell<Option<N>>, &'s Cell<Option<N>>),
{
	fn sentinel(&self) -> Option<N> {
		None
	}

	fn is_sentinel(&self, node: Option<N>) -> bool {
		node.is_none()
	}

	fn get_prev(&self, node: Option<N>) -> Option<N> {
		self.get_prev_cell(node).get()
	}

	fn get_next(&self, node: Option<N>) -> Option<N> {
		self.get_next_cell(node).get()
	}

	fn set_prev(&mut self, node: Option<N>, val: Option<N>) {
		self.get_prev_cell(node).set(val);
	}

	fn set_next(&mut self, node: Option<N>, val: Option<N>) {
		self.get_next_cell(node).set(val);
	}
}
