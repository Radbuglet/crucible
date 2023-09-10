use std::{borrow::Cow, marker::PhantomData};

use bort::{Entity, OwnedEntity};
use crucible_util::{
	lang::marker::PhantomInvariant,
	mem::{free_list::FreeList, hash::FxHashMap},
};
use derive_where::derive_where;

pub trait MaterialMarker: 'static {}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct RawMaterialId(pub u16);

impl RawMaterialId {
	pub const AIR: Self = Self(0);
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MaterialId<M: MaterialMarker> {
	_ty: PhantomInvariant<M>,
	raw: RawMaterialId,
}

impl<M: MaterialMarker> MaterialId<M> {
	pub const AIR: Self = Self {
		_ty: PhantomData,
		raw: RawMaterialId::AIR,
	};
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MaterialInfo<M: MaterialMarker> {
	pub id: MaterialId<M>,
	pub descriptor: Entity,
}

#[derive_where(Debug, Default)]
pub struct MaterialRegistry<M: MaterialMarker> {
	slots: FreeList<OwnedEntity, u16>,
	name_map: FxHashMap<Cow<'static, str>, MaterialInfo<M>>,
}

impl<M: MaterialMarker> MaterialRegistry<M> {
	pub fn register(
		&mut self,
		name: impl Into<Cow<'static, str>>,
		descriptor: OwnedEntity,
	) -> MaterialInfo<M> {
		// Register in slot store
		let descriptor_ref = descriptor.entity();
		let (_, slot) = self.slots.add(descriptor);

		// Construct material
		let material_id = MaterialId {
			_ty: PhantomData,
			raw: RawMaterialId(slot),
		};
		let material = MaterialInfo {
			id: material_id,
			descriptor: descriptor_ref,
		};

		// Register in map
		let name = name.into();
		let name_clone = name.clone();

		if let Err(e) = self.name_map.try_insert(name, material) {
			log::error!("Registered duplicate material with id {:?}.", e.entry.key());
		}

		// Attach `MaterialDescriptorBase`
		descriptor_ref.insert(MaterialDescriptorBase {
			name: name_clone,
			slot: material_id.raw,
		});

		material
	}

	pub fn unregister(&mut self, target: Entity) {
		let MaterialDescriptorBase { name, slot } = target.remove().unwrap();
		self.name_map.remove(&name);
		self.slots.remove(slot.0);
	}

	pub fn find_by_name(&self, name: &str) -> Option<MaterialInfo<M>> {
		self.name_map.get(name).copied()
	}

	pub fn find_by_id(&self, id: MaterialId<M>) -> MaterialInfo<M> {
		let descriptor = self.slots.get(id.raw.0).entity();
		MaterialInfo { id, descriptor }
	}
}

#[derive(Debug, Clone)]
pub struct MaterialDescriptorBase {
	name: Cow<'static, str>,
	slot: RawMaterialId,
}

impl MaterialDescriptorBase {
	pub fn name(&self) -> &str {
		self.name.as_ref()
	}

	pub fn material(&self) -> RawMaterialId {
		self.slot
	}
}
