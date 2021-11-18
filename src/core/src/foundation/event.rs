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
	queue: VecDeque<E>,
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
		self.queue.push_back(event);
	}

	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I)
	where
		<I as IntoIterator>::IntoIter: 'static,
	{
		for ev in iter {
			self.push(ev);
		}
	}
}

pub struct EventPollDrain<'a, E> {
	target: &'a mut EventPusherPoll<E>,
}

impl<'a, E> Iterator for EventPollDrain<'a, E> {
	type Item = E;

	fn next(&mut self) -> Option<Self::Item> {
		self.target.queue.pop_front()
	}
}

// pub struct EventPusherPoll<E, X> {
// 	//_ty: PhantomData<fn(X) -> X>,
// 	queue: VecDeque<PollEntry<E, X>>,
// }
//
// impl<E, X> EventPusherPoll<E, X>
// where
// 	X: FnMut(&mut E) -> bool,
// {
// 	pub fn new() -> Self {
// 		Self {
// 			//_ty: PhantomData,
// 			queue: VecDeque::new(),
// 		}
// 	}
//
// 	pub fn handle(&mut self, mut exec: X) -> bool {
// 		loop {
// 			match self.queue.front_mut() {
// 				Some(PollEntry::Entry(entry)) => {
// 					if !exec(entry) {
// 						return false;
// 					}
// 				}
// 				Some(PollEntry::Iter(iter)) => {
// 					if !iter.run(&mut exec) {
// 						return false;
// 					}
// 				}
// 				None => return true,
// 			}
// 			self.queue.pop_front();
// 		}
// 	}
// }
//
// enum PollEntry<E, X> {
// 	Entry(E),
// 	Iter(Box<dyn PollIter<Elem = E, Handler = X>>),
// }
//
// trait PollIter {
// 	type Elem;
// 	type Handler: FnMut(Self::Elem) -> bool;
//
// 	fn run(&mut self, handler: &mut Self::Handler) -> bool;
// }
//
// struct PollIterGeneric<I, X> {
// 	_ty: PhantomData<fn(X) -> X>,
// 	iter: I,
// }
//
// impl<I, X> PollIter for PollIterGeneric<I, X>
// where
// 	I: Iterator,
// 	X: FnMut(&mut I::Item) -> bool,
// {
// 	type Elem = I::Item;
// 	type Handler = X;
//
// 	fn run(&mut self, handler: &mut Self::Handler) -> bool {
// 		while let Some(next) = self.iter.next() {
// 			if !handler(next) {
// 				return false;
// 			}
// 		}
// 		true
// 	}
// }
//
// impl<E, X> EventPusher for EventPusherPoll<E, X>
// where
// 	X: FnMut(&mut E) -> bool,
// {
// 	type Event = E;
//
// 	fn push(&mut self, event: Self::Event) {
// 		self.queue.push_back(PollEntry::Entry(event));
// 	}
//
// 	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I)
// 	where
// 		<I as IntoIterator>::IntoIter: 'static,
// 	{
// 		self.queue
// 			.push_back(PollEntry::Iter(Box::new(PollIterGeneric {
// 				_ty: PhantomData,
// 				iter,
// 			}) as Box<_>));
// 	}
// }
