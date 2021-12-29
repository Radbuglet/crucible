use std::collections::VecDeque;
use std::marker::PhantomData;

// === Core === //

pub trait EventPusher {
	type Event;

	fn push(&mut self, event: Self::Event);

	// TODO: Reduce required iterator lifetime.
	fn push_iter<I: IntoIterator<Item = Self::Event>>(&mut self, iter: I)
	where
		<I as IntoIterator>::IntoIter: 'static,
	{
		for elem in iter.into_iter() {
			self.push(elem);
		}
	}
}

// === Immediate === //

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
}

// === Deque === //

impl<E> EventPusher for VecDeque<E> {
	type Event = E;

	fn push(&mut self, event: Self::Event) {
		self.push_back(event);
	}
}

// === Callback Poll === //

pub struct EventPusherCallback<E, X> {
	queue: VecDeque<PollEntry<E, X>>,
}

enum PollEntry<E, X> {
	Entry(E),
	Iter(Box<dyn PollCallback<X>>),
}

impl<E, X> EventPusherCallback<E, X>
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

impl<E, X> EventPusher for EventPusherCallback<E, X>
where
	X: FnMut(E) -> bool,
{
	type Event = E;

	fn push(&mut self, event: Self::Event) {
		self.queue.push_back(PollEntry::Entry(event));
	}

	fn push_iter<I>(&mut self, iter: I)
	where
		I: IntoIterator<Item = E>,
		<I as IntoIterator>::IntoIter: 'static,
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
