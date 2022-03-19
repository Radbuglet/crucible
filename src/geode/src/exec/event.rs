// use crate::ecs::map::{Entity, MapStorage, World};
use std::collections::VecDeque;

pub trait EventTarget<'i, E> {
	fn fire(&mut self, event: E);

	fn fire_iter<I: IntoIterator<Item = E>>(&mut self, iter: I)
	where
		<I as IntoIterator>::IntoIter: 'i,
	{
		for elem in iter.into_iter() {
			self.fire(elem);
		}
	}
}

impl<'i, E, O> EventTarget<'i, E> for O
where
	O: FnMut(E),
{
	fn fire(&mut self, event: E) {
		self(event)
	}
}

impl<'i, E> EventTarget<'i, E> for VecDeque<E> {
	fn fire(&mut self, event: E) {
		self.push_back(event);
	}

	fn fire_iter<I: IntoIterator<Item = E>>(&mut self, iter: I)
	where
		I::IntoIter: 'i,
	{
		self.extend(iter)
	}
}

// #[derive(Debug, Clone, Hash, Eq, PartialEq)]
// pub struct EntityEvent<'a, E> {
//     pub world: &'a World,
//     pub target: Entity,
//     pub event: E,
// }
//
// impl<'w, E> EventTarget<'_, EntityEvent<'w, E>> for MapStorage<E> {
//     fn fire(&mut self, event: EntityEvent<'w, E>) {
//         self.insert(event.world, event.target, event.event);
//     }
// }

pub struct EventTargetCallback<'i, E, X> {
	queue: VecDeque<PollEntry<'i, E, X>>,
}

enum PollEntry<'i, E, X> {
	Entry(E),
	Iter(Box<dyn PollCallback<X> + 'i>),
}

impl<'i, E, X> EventTargetCallback<'i, E, X>
where
	X: FnMut(E) -> bool,
{
	pub fn new() -> Self {
		Self {
			queue: VecDeque::new(),
		}
	}

	pub fn handle(&mut self, mut exec: X) -> bool {
		loop {
			match self.queue.pop_front() {
				Some(PollEntry::Entry(entry)) => {
					if !exec(entry) {
						return false;
					}
				}
				Some(PollEntry::Iter(mut iter)) => {
					if !iter.run(&mut exec) {
						self.queue.push_front(PollEntry::Iter(iter));
						return false;
					}
				}
				None => return true,
			}
		}
	}
}

impl<'i, E, X> EventTarget<'i, E> for EventTargetCallback<'i, E, X>
where
	X: FnMut(E) -> bool,
{
	fn fire(&mut self, event: E) {
		self.queue.push_back(PollEntry::Entry(event));
	}

	fn fire_iter<I>(&mut self, iter: I)
	where
		I: IntoIterator<Item = E>,
		<I as IntoIterator>::IntoIter: 'i,
	{
		self.queue
			.push_back(PollEntry::Iter(Box::new(PollIterCallback {
				iter: iter.into_iter(),
			}) as Box<dyn PollCallback<X>>));
	}
}

trait PollCallback<H> {
	fn run(&mut self, handler: &mut H) -> bool;
}

struct PollIterCallback<I> {
	iter: I,
}

impl<I, X> PollCallback<X> for PollIterCallback<I>
where
	I: Iterator,
	X: FnMut(I::Item) -> bool,
{
	fn run(&mut self, handler: &mut X) -> bool {
		while let Some(next) = self.iter.next() {
			if !handler(next) {
				return false;
			}
		}
		true
	}
}

// pub trait NestedEvent {
//     type Event;
//     type DirtyQuery;
//     type EventQuery;
//
//     fn dirty_query(&self) -> Self::DirtyQuery;
//     fn event_query(&self) -> Self::EventQuery;
// }
//
// pub struct SimpleEventPath {}
//
// pub struct ListenerTree {
//     listens_to: MapStorage<()>,
// }

pub trait DynEventTarget {
	// ...
}
