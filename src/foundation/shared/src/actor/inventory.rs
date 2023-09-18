use bort::{Entity, EventTarget, HasGlobalManagedTag};

#[derive(Debug)]
pub enum InventoryUpdated {
	StackSet(usize, Option<Entity>, Option<Entity>),
	StacksSwapped(usize, usize),
	StackDestroyed(Entity),
}

#[derive(Debug)]
pub struct InventoryData {
	slots: Vec<Option<Entity>>,
}

impl HasGlobalManagedTag for InventoryData {
	type Component = Self;
}

impl InventoryData {
	pub fn new(slots: usize) -> Self {
		Self {
			slots: (0..slots).map(|_| None).collect(),
		}
	}

	pub fn slot(&self, index: usize) -> Option<Entity> {
		self.slots[index]
	}

	pub fn set_slot(
		&mut self,
		me: Entity,
		on_inventory_changed: &mut impl EventTarget<InventoryUpdated>,
		index: usize,
		stack: Option<Entity>,
	) {
		let old = self.slot(index);
		self.slots[index] = stack;

		on_inventory_changed.fire(me, InventoryUpdated::StackSet(index, old, stack), ());

		if let Some(old) = old {
			on_inventory_changed.fire(me, InventoryUpdated::StackDestroyed(old), ());
		}
	}

	pub fn swap_slots(
		&mut self,
		me: Entity,
		on_inventory_changed: &mut impl EventTarget<InventoryUpdated>,
		index_a: usize,
		index_b: usize,
	) {
		if index_a != index_b {
			self.slots.swap(index_a, index_b);
			on_inventory_changed.fire(me, InventoryUpdated::StacksSwapped(index_a, index_b), ());
		}
	}

	#[must_use]
	pub fn insert_stack(
		&mut self,
		me: Entity,
		on_inventory_changed: &mut impl EventTarget<InventoryUpdated>,
		stack: Entity,
		mut merge: impl FnMut(Entity, Entity) -> bool,
	) -> Option<Entity> {
		for (i, target_stack) in self.slots.iter().enumerate() {
			if let Some(target_stack) = target_stack {
				if merge(stack, *target_stack) {
					return None;
				}
			} else {
				self.set_slot(me, on_inventory_changed, i, Some(stack));
				return None;
			}
		}

		Some(stack)
	}

	pub fn clear(
		&mut self,
		me: Entity,
		on_inventory_changed: &mut impl EventTarget<InventoryUpdated>,
	) {
		for slot in &mut self.slots {
			if let Some(stack) = slot.take() {
				on_inventory_changed.fire(me, InventoryUpdated::StackDestroyed(stack), ());
			}
		}
	}
}
