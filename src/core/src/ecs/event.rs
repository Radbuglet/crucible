use std::{any::type_name, mem, vec};

use derive_where::derive_where;
use hashbrown::HashMap;

use crate::debug::lifetime::{DebugLifetime, Dependent};

use super::entity::{ArchetypeId, ArchetypeMap, Entity};

// === Aliases === //

#[derive(Debug, Clone, Default)]
pub struct EntityDestroyEvent;

pub type DestroyQueue = EventQueue<EntityDestroyEvent>;

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

	pub fn flush_all(&mut self) -> impl Iterator<Item = EventQueueIter<E>> {
		mem::replace(&mut self.runs, HashMap::new())
			.into_iter()
			.map(|(arch, events_list)| EventQueueIter(arch.into_inner(), events_list.into_iter()))
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

impl<E> EventQueueIter<E> {
	pub fn arch(&self) -> ArchetypeId {
		self.0
	}
}

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

#[derive(Debug)]
#[derive_where(Default)]
pub struct TaskQueue<T> {
	task_stack: Vec<T>,
	tasks_to_add: Vec<T>,
}

impl<T> TaskQueue<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push(&mut self, task: T) {
		// These are queued in a separate buffer and moved into the main buffer during `next_task`
		// to ensure that tasks are pushed in an intuitive order.
		self.tasks_to_add.push(task);
	}

	pub fn next_task(&mut self) -> Option<T> {
		// Move all tasks from `tasks_to_add` to `task_stack`. This flips their order, which is
		// desireable.
		self.task_stack.reserve(self.tasks_to_add.len());
		while let Some(to_add) = self.tasks_to_add.pop() {
			self.task_stack.push(to_add);
		}

		// Now, pop off the next task to be ran.
		self.task_stack.pop()
	}

	pub fn clear_capacities(&mut self) {
		self.task_stack = Vec::new();
		self.tasks_to_add = Vec::new();
	}
}

impl<T> Drop for TaskQueue<T> {
	fn drop(&mut self) {
		let remaining = self.task_stack.len() + self.tasks_to_add.len();

		if remaining > 0 {
			log::warn!(
				"Leaked {} event{} on the TaskQueue.",
				remaining,
				if remaining == 1 { "" } else { "s" },
			);
		}
	}
}
