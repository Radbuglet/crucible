use bort::OwnedEntity;
use crucible_util::mem::array::boxed_arr_from_fn;
use typed_glam::glam::IVec2;

#[derive(Debug)]
pub struct Inventory {
	size: IVec2,
	slots: Box<[ItemStack]>,
}

impl Inventory {
	pub fn new(size: IVec2, extra_slots: u32) -> Self {
		Self {
			size,
			slots: boxed_arr_from_fn(
				ItemStack::default,
				(size.x * size.y + extra_slots as i32) as usize,
			),
		}
	}

	pub fn size(&self) -> IVec2 {
		self.size
	}

	pub fn index_of_slot(&self, pos: IVec2) -> usize {
		debug_assert!(pos.cmple(self.size).all());

		(self.size.x * pos.y + pos.y) as usize
	}

	pub fn index_of_extra_slot(&self, index: u32) -> usize {
		(self.size.x * self.size.y) as usize + index as usize
	}

	pub fn slot(&self, pos: IVec2) -> &ItemStack {
		&self.slots[self.index_of_slot(pos)]
	}

	pub fn slot_mut(&mut self, pos: IVec2) -> &mut ItemStack {
		&mut self.slots[self.index_of_slot(pos)]
	}

	pub fn extra_slot(&self, index: u32) -> &ItemStack {
		&self.slots[self.index_of_extra_slot(index)]
	}

	pub fn extra_slot_mut(&mut self, index: u32) -> &mut ItemStack {
		&mut self.slots[self.index_of_extra_slot(index)]
	}
}

#[derive(Debug, Default)]
pub struct ItemStack {
	pub id: u16,
	pub count: u16,
	pub meta: Option<OwnedEntity>,
}
