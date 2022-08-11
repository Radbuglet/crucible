use std::{fmt, num::NonZeroU8};

use super::{
	debug::DebugLabel,
	internals::db,
	owned::{Destructible, Owned},
	session::Session,
};

// === Lock Management === //

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(NonZeroU8);

impl fmt::Debug for Lock {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Lock")
			.field("slot", &self.slot())
			.field("debug_name", &db::get_lock_debug_name(self.slot()))
			.finish()
	}
}

impl Destructible for Lock {
	fn destruct(self) {
		db::unreserve_lock(self.slot())
	}
}

impl Lock {
	pub fn new<L: DebugLabel>(label: L) -> Owned<Self> {
		let id = db::reserve_lock(label.to_debug_label());
		Owned::new(Lock(id))
	}

	pub fn is_held(self) -> bool {
		db::is_lock_held_somewhere(self.slot())
	}

	pub fn slot(self) -> NonZeroU8 {
		self.0
	}
}

impl Session<'_> {
	pub fn acquire_locks<I: IntoIterator<Item = Lock>>(self, locks: I) {
		db::acquire_locks(
			self,
			&locks
				.into_iter()
				.map(|lock| lock.slot())
				.collect::<Vec<_>>(),
		);
	}

	pub fn reserve_slot_capacity(self, amount: usize) {
		db::reserve_obj_slot_capacity(self, amount)
	}
}

// === Lock Math === //
