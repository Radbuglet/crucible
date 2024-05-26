#[derive(Debug, Clone, Default)]
pub struct SlotAssigner {
	next_slot: u32,
}

impl SlotAssigner {
	pub fn jump_to(&mut self, slot: u32) {
		self.next_slot = slot;
	}

	pub fn peek(&self) -> u32 {
		self.next_slot
	}

	pub fn next(&mut self) -> u32 {
		let binding = self.next_slot;
		self.next_slot = self
			.next_slot
			.checked_add(1)
			.expect("Cannot create a binding at slot `u32::MAX`.");

		binding
	}
}
