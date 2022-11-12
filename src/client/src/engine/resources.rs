use crucible_core::{
	debug::userdata::UserdataValue,
	ecs::{core::Entity, userdata::Userdata},
	lang::polyfill::{BuildHasherPoly, OptionPoly},
};
use hashbrown::raw::RawTable;

use std::{collections::hash_map::RandomState, hash::Hash};

pub trait ResourceDescriptor: 'static + UserdataValue + Hash + Eq + Clone {
	type Context<'a>;

	fn construct(&mut self, cx: Self::Context<'_>) -> Entity;
}

#[derive(Default)]
pub struct ResourceManager {
	hasher: RandomState,
	resources: RawTable<ReifiedResource>,
}

#[derive(Debug)]
struct ReifiedResource {
	desc_hash: u64,
	desc: Userdata,
	res: Entity,
}

impl ResourceManager {
	pub fn load<C, D: ResourceDescriptor>(&mut self, desc: &D, cx: D::Context<'_>) -> Entity {
		if let Some(res) = self.find(desc) {
			return res;
		}

		let mut desc = Box::new(desc.clone());
		let res = desc.construct(cx);
		let desc_hash = self.hasher.hash_one(&desc);

		self.resources.insert(
			desc_hash,
			ReifiedResource {
				desc_hash,
				desc,
				res,
			},
			|res| res.desc_hash,
		);

		res
	}

	pub fn find<D: ResourceDescriptor>(&self, desc: &D) -> Option<Entity> {
		let hash = self.hasher.hash_one(desc);
		let resource = self.resources.get(hash, |res| {
			res.desc_hash == hash
				&& res
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|desc_rhs| desc == desc_rhs)
		})?;

		Some(resource.res)
	}
}
