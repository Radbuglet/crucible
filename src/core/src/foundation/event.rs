use std::collections::VecDeque;
use std::marker::PhantomData;

pub trait EventPusher {
	type Event;

	fn push(&mut self, event: Self::Event);
	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I)
	where
		<I as IntoIterator>::IntoIter: 'static; // TODO: Reduce required iterator lifetime.
}

pub struct EventPusherImmediate<E, F> {
	// For some reason, the FnMut trait argument types do not behave like associated types so we
	// have to make the pusher generic over the event and the function.
	// Variance: covariant
	_ty: PhantomData<fn(E)>,
	handler: F,
}

// We add the F binding here, despite it not being strictly necessary, to allow users to construct
// unused `EventPusherImmediate` instances without having to fully qualify the event type.
impl<E, F: FnMut(E)> EventPusherImmediate<E, F> {
	pub fn new(handler: F) -> Self {
		Self {
			_ty: PhantomData,
			handler,
		}
	}
}

impl<E, F: FnMut(E)> EventPusher for EventPusherImmediate<E, F> {
	type Event = E;

	fn push(&mut self, event: Self::Event) {
		(self.handler)(event);
	}

	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I) {
		for event in iter {
			(self.handler)(event);
		}
	}
}

pub struct EventPusherPoll<E> {
	// TODO: Optimize this!
	queue: VecDeque<PollEntry<E>>,
}

enum PollEntry<E> {
	Entry(E),
	// TODO: Optimize this! (repeated `dyn` calls)
	Iter(Box<dyn Iterator<Item = E>>),
}

impl<E> EventPusherPoll<E> {
	pub fn new() -> Self {
		Self {
			queue: VecDeque::new(),
		}
	}

	pub fn drain(&mut self) -> EventPollDrain<E> {
		EventPollDrain { target: self }
	}
}

impl<E> EventPusher for EventPusherPoll<E> {
	type Event = E;

	fn push(&mut self, event: Self::Event) {
		self.queue.push_back(PollEntry::Entry(event));
	}

	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I)
	where
		<I as IntoIterator>::IntoIter: 'static,
	{
		self.queue.push_back(PollEntry::Iter(
			Box::new(iter.into_iter()) as Box<dyn Iterator<Item = E>>
		));
	}
}

pub struct EventPollDrain<'a, E> {
	target: &'a mut EventPusherPoll<E>,
}

impl<'a, E> Iterator for EventPollDrain<'a, E> {
	type Item = E;

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			match self.target.queue.pop_front() {
				Some(PollEntry::Entry(entry)) => break Some(entry),
				Some(PollEntry::Iter(mut iter)) => {
					if let Some(event) = iter.next() {
						self.target.queue.push_front(PollEntry::Iter(iter));
						break Some(event);
					}
				}
				None => break None,
			}
		}
	}
}
