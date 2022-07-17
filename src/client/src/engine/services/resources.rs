use std::{
	any::Any,
	collections::hash_map::RandomState,
	error::Error,
	hash::{self, BuildHasher, Hasher},
};

use hashbrown::raw::RawTable;

use crucible_core::c_enum::{c_enum, CEnumMap};
use geode::prelude::*;

// === CostCategory === //

c_enum! {
	pub enum CostCategory {
		AssetCount,
		CpuMemory,
		GpuMemory,
		GpuTextureCount,
		GpuBufferCount,
		GpuPipelineCount,
	}
}

// === ManagedResourceAliveQuery === //

event_trait! {
	pub trait ManagedResourceAliveQuery::should_keep_alive(&self, event: ShouldKeepAliveEvent);
}

#[derive(Debug, Clone)]
pub struct ShouldKeepAliveEvent<'a> {
	pub session: Session<'a>,
	pub me: Entity,
	verdict: Option<bool>,
}

impl<'a> ShouldKeepAliveEvent<'a> {
	pub fn support(&mut self) {
		self.verdict = self.verdict.or(Some(true));
	}

	pub fn condemn(&mut self) {
		self.verdict = Some(false);
	}

	pub fn verdict(&self) -> Option<bool> {
		self.verdict
	}
}

// === ResourceManager === //

#[derive(Default)]
pub struct ResourceManager {
	resource_map: RawTable<(u64, ManagedResource)>,
	// total_cost: CEnumMap<CostCategory, u64>,
	hash_builder: RandomState,
}

struct ManagedResource {
	descriptor: Owned<Obj<dyn Any + Send>>,
	resource: Owned<Entity>,
	// cost: CEnumMap<CostCategory, u64>,
}

impl ResourceManager {
	pub fn try_load<D: ResourceDescriptor>(
		&mut self,
		s: Session,
		ctx: D::Context,
		descriptor: D,
	) -> Result<EntityWith<D>, D::CreationError> {
		// Find existing resource
		let hash = {
			let mut hasher = self.hash_builder.build_hasher();
			descriptor.hash(&mut hasher);
			hasher.finish()
		};

		if let Some((_, res)) = self.resource_map.get(hash, |(descriptor_hash, entry)| {
			if hash != *descriptor_hash {
				return false;
			}

			if !matches!(entry.descriptor.get(s).downcast_ref::<D>(), Some(rhs_descriptor) if &descriptor == rhs_descriptor)
			{
				return false;
			}

			true
		}) {
			Ok(EntityWith::unchecked_cast(res.resource.weak_copy()))
		} else {
			// TODO: Check hard GC limits.

			let (resource, _cost) = descriptor.create(s, ctx)?;
			let resource_weak = resource.weak_copy();
			let resource = resource.raw();
			let descriptor = descriptor.box_obj(s).cast::<dyn Any + Send>();

			self.resource_map.insert(hash, (hash, ManagedResource {
				descriptor,
				resource,
			}), |(hash, _)| *hash);

			Ok(resource_weak)
		}
	}

	pub fn gc(&mut self) {
		// TODO: lol no gc
	}
}

pub trait ResourceDescriptor: ObjPointee + hash::Hash + Eq + Sync {
	type CreationError: Error;
	type Resource: ObjPointee;
	type Context;

	fn create(
		&self,
		s: Session,
		ctx: Self::Context,
	) -> Result<(Owned<EntityWith<Self>>, CEnumMap<CostCategory, u64>), Self::CreationError>;
}

// === Standard Validator Components === //

// TODO
