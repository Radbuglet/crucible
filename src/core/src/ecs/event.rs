use std::{any::type_name, fmt, mem, vec};

use derive_where::derive_where;

use crate::{
	debug::{
		lifetime::{DebugLifetime, Dependent},
		userdata::Userdata,
	},
	lang::explicitly_bind::ExplicitlyBind,
};

use super::{
	entity::{ArchetypeId, ArchetypeMap, Entity},
	provider::DynProvider,
};

// === Aliases === //

pub type EventHandlerFn<T> = fn(&mut DynProvider, EventQueueIter<T>);
pub type EventHandlerMap<T> = ArchetypeMap<EventHandlerFn<T>>;

#[derive(Debug, Copy, Clone, Default)]
pub struct EntityDestroyEvent;

pub type DestroyQueue = EventQueue<EntityDestroyEvent>;
pub type DestroyHandlerMap = EventHandlerMap<EntityDestroyEvent>;

// === EventQueue === //

#[derive(Debug, Clone)]
#[derive_where(Default)]
pub struct EventQueue<E> {
	runs: ArchetypeMap<Vec<Event<E>>>,
	maybe_recursively_dispatched: bool,
}

impl<E> EventQueue<E> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push(&mut self, target: Entity, event: E) {
		let run = if let Some(run) = self.runs.get_mut(&target.arch) {
			run
		} else {
			self.maybe_recursively_dispatched = true;

			self.runs
				.insert_unique_unchecked(Dependent::new(target.arch), Vec::new())
				.1
		};

		run.push(Event {
			slot: target.slot,
			lifetime: Dependent::new(target.lifetime),
			event,
		});
	}

	pub fn flush_in(&mut self, archetype: ArchetypeId) -> EventQueueIter<E> {
		EventQueueIter(
			archetype,
			self.runs
				.remove(&archetype)
				.unwrap_or(Vec::new())
				.into_iter(),
		)
	}

	pub fn maybe_recursively_dispatched(&mut self) -> bool {
		mem::replace(&mut self.maybe_recursively_dispatched, false)
	}

	pub fn is_empty(&self) -> bool {
		self.runs.is_empty()
	}

	pub fn has_remaining(&self) -> bool {
		!self.is_empty()
	}
}

impl<E> Drop for EventQueue<E> {
	fn drop(&mut self) {
		if !self.runs.is_empty() {
			let leaked_count = self.runs.values().map(|run| run.len()).sum::<usize>();

			log::error!(
				"Leaked {leaked_count} event{} from {}",
				if leaked_count == 1 { "" } else { "s" },
				type_name::<Self>()
			);
		}
	}
}

#[derive(Debug, Clone)]
struct Event<E> {
	slot: u32,
	lifetime: Dependent<DebugLifetime>,
	event: E,
}

impl<E> Event<E> {
	fn into_tuple(self, arch: ArchetypeId) -> (Entity, E) {
		(
			Entity {
				slot: self.slot,
				lifetime: self.lifetime.get(),
				arch,
			},
			self.event,
		)
	}
}

#[derive(Debug, Clone)]
pub struct EventQueueIter<E>(ArchetypeId, vec::IntoIter<Event<E>>);

impl<E> Iterator for EventQueueIter<E> {
	type Item = (Entity, E);

	fn next(&mut self) -> Option<Self::Item> {
		self.1.next().map(|e| e.into_tuple(self.0))
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.1.size_hint()
	}

	fn count(self) -> usize {
		self.1.count()
	}
}

impl<E> ExactSizeIterator for EventQueueIter<E> {}

impl<E> DoubleEndedIterator for EventQueueIter<E> {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.1.next_back().map(|e| e.into_tuple(self.0))
	}
}

// === TaskQueue === //

#[derive(Debug, Default)]
pub struct TaskQueue {
	events: Vec<Box<dyn Task>>,
}

trait Task: Userdata {
	fn fire(&mut self, scheduler: &mut TaskQueue, cx: &mut DynProvider);
}

struct TaskWrapper<F>(ExplicitlyBind<F>);

impl<F: fmt::Debug> fmt::Debug for TaskWrapper<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self.0, f)
	}
}

impl<F> Task for TaskWrapper<F>
where
	F: FnOnce(&mut TaskQueue, &mut DynProvider) + Userdata,
{
	fn fire(&mut self, bus: &mut TaskQueue, cx: &mut DynProvider) {
		ExplicitlyBind::extract(&mut self.0)(bus, cx);
	}
}

impl TaskQueue {
	pub fn push<F>(&mut self, handler: F)
	where
		F: FnOnce(&mut TaskQueue, &mut DynProvider) + Userdata,
	{
		log::info!("Pushing event {handler:?}.");
		let handler = Box::new(TaskWrapper(ExplicitlyBind::new(handler)));
		self.events.push(handler);
	}

	pub fn dispatch(&mut self, cx: &mut DynProvider) {
		while !self.events.is_empty() {
			for mut event in mem::replace(&mut self.events, Vec::new()) {
				log::trace!("Executing event {event:?}.");
				event.fire(self, cx);
			}
		}
	}
}

impl Drop for TaskQueue {
	fn drop(&mut self) {
		if !self.events.is_empty() {
			log::warn!(
				"Leaked {} event{} on the TaskQueue: {:#?}",
				self.events.len(),
				if self.events.len() == 1 { "" } else { "s" },
				self.events
			);
		}
	}
}
