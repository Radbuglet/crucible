use crate::actor::inventory::Inventory;

#[derive(Debug)]
pub struct PlayerCommon {
	pub inventory: Inventory,
	pub hotbar_slot: u32,
}
