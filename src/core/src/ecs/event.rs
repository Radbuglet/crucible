use std::{fmt, mem};

use crate::{debug::userdata::UserdataValue, lang::explicitly_bind::ExplicitlyBind};

use super::provider::DynProvider;

// === Scheduler === //

#[derive(Debug, Default)]
pub struct Scheduler {
	events: Vec<Box<dyn Task>>,
}

trait Task: UserdataValue {
	fn fire(&mut self, scheduler: &mut Scheduler, cx: &mut DynProvider);
}

struct TaskWrapper<F>(ExplicitlyBind<F>);

impl<F: fmt::Debug> fmt::Debug for TaskWrapper<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self.0, f)
	}
}

impl<F> Task for TaskWrapper<F>
where
	F: FnOnce(&mut Scheduler, &mut DynProvider) + UserdataValue,
{
	fn fire(&mut self, bus: &mut Scheduler, cx: &mut DynProvider) {
		ExplicitlyBind::extract(&mut self.0)(bus, cx);
	}
}

impl Scheduler {
	pub fn push<F>(&mut self, handler: F)
	where
		F: FnOnce(&mut Scheduler, &mut DynProvider) + UserdataValue,
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

impl Drop for Scheduler {
	fn drop(&mut self) {
		if !self.events.is_empty() {
			log::warn!(
				"Leaked {} event{} on the EventBus: {:#?}",
				self.events.len(),
				if self.events.len() == 1 { "" } else { "s" },
				self.events
			);
		}
	}
}
