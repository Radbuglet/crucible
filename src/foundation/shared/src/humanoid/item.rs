use crate::material::{MaterialId, MaterialInfo, MaterialMarker, MaterialRegistry};

// === ItemMaterialRegistry === //

#[non_exhaustive]
pub struct ItemMaterialMarker;

impl MaterialMarker for ItemMaterialMarker {}

pub type ItemMaterialRegistry = MaterialRegistry<ItemMaterialMarker>;
pub type ItemMaterialInfo = MaterialInfo<ItemMaterialMarker>;
pub type ItemMaterialId = MaterialId<ItemMaterialMarker>;

// === ItemStackBase === //

#[derive(Debug, Clone)]
pub struct ItemStackBase {
	pub material: ItemMaterialId,
	pub count: u16,
}
