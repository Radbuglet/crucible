#![allow(dead_code)]

use crucible_core::{
	debug::userdata::{ErasedUserdataValue, Userdata, UserdataValue},
	lang::{
		loan::downcast_userdata_arc,
		polyfill::{BuildHasherPoly, OptionPoly},
	},
};

use hashbrown::HashMap;

use std::{hash::Hash, sync::Arc};

pub trait ResourceDescriptor: 'static + UserdataValue + Hash + Eq + Clone {
	type Context<'a>;
	type Resource: UserdataValue;

	fn construct(
		&self,
		res_mgr: &mut ResourceManager,
		cx: Self::Context<'_>,
	) -> Arc<Self::Resource>;

	fn keep_alive(&self, res_mgr: &mut ResourceManager) {
		let _ = res_mgr;
		// (no op)
	}
}

#[derive(Debug, Default)]
pub struct ResourceManager {
	resources: HashMap<ReifiedDescriptor, Option<Arc<dyn UserdataValue>>>,
	// TODO: Implement automatic cleanup
}

#[derive(Debug)]
struct ReifiedDescriptor {
	hash: u64,
	desc: Userdata,
}

impl ResourceManager {
	pub fn load<D: ResourceDescriptor>(
		&mut self,
		desc: &D,
		cx: D::Context<'_>,
	) -> Arc<D::Resource> {
		// Try to reuse an existing resource.
		if let Some(res) = self.find(desc) {
			return res;
		}

		// Insert an unfinished resource stub into the registry. This is used to detect cyclic
		// resource dependencies.
		let hash = self.resources.hasher().p_hash_one(&desc);
		let reified_desc = ReifiedDescriptor {
			hash,
			desc: Box::new(desc.clone()),
		};

		self.resources
			.raw_table()
			.insert(hash, (reified_desc, None), |(desc, _)| desc.hash);

		// Construct the resource
		let res = desc.construct(self, cx);

		// Update the stub to contain the resource.
		let (_, stub) = self
			.resources
			.raw_table()
			.get_mut(hash, |(candidate, _)| {
				candidate
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|desc_rhs| desc == desc_rhs)
			})
			.unwrap();

		*stub = Some(res);

		// Convert it into an `Arc<D::Resource>`
		let res = stub.as_ref().unwrap();
		downcast_userdata_arc(res.clone())
	}

	pub fn find<D: ResourceDescriptor>(&self, desc: &D) -> Option<Arc<D::Resource>> {
		let hash = self.resources.hasher().p_hash_one(desc);
		let (_, res) = self.resources.raw_entry().from_hash(hash, |candidate| {
			candidate.hash == hash
				&& candidate
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|candidate| desc == candidate)
		})?;

		let res = res
			.as_ref()
			.unwrap_or_else(|| panic!("Detected recursive dependency on dependency {desc:?}."));

		Some(downcast_userdata_arc(res.clone()))
	}

	pub fn keep_alive<D: ResourceDescriptor>(&mut self, desc: &D) {
		let _ = desc;
		todo!()
	}
}
