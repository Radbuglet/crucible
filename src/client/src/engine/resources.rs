use crucible_core::{
	debug::userdata::{downcast_userdata_arc, Userdata, UserdataValue},
	lang::polyfill::{BuildHasherPoly, OptionPoly},
};

use hashbrown::raw::RawTable;

use std::{collections::hash_map::RandomState, hash::Hash, sync::Arc};

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

#[derive(Default)]
pub struct ResourceManager {
	hasher: RandomState,
	resources: RawTable<ReifiedResource>,
	// TODO: Implement automatic cleanup
}

#[derive(Debug)]
struct ReifiedResource {
	desc_hash: u64,
	desc: Userdata,
	// `None` if the resource hasn't loaded yet.
	res: Option<Arc<dyn UserdataValue>>,
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
		let desc_hash = self.hasher.hash_one(&desc);

		self.resources.insert(
			desc_hash,
			ReifiedResource {
				desc_hash,
				desc: Box::new(desc.clone()),
				res: None,
			},
			|res| res.desc_hash,
		);

		// Construct the resource
		let res = desc.construct(self, cx);

		// Update the stub to contain the resource.
		let stub = self
			.resources
			.get_mut(desc_hash, |candidate| {
				candidate
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|desc_rhs| desc == desc_rhs)
			})
			.unwrap();

		stub.res = Some(res);

		// Convert it into an `Arc<D::Resource>`
		let res = stub.res.as_ref().unwrap();
		downcast_userdata_arc(res.clone())
	}

	pub fn find<D: ResourceDescriptor>(&self, desc: &D) -> Option<Arc<D::Resource>> {
		let hash = self.hasher.hash_one(desc);
		let entry = self.resources.get(hash, |res| {
			res.desc_hash == hash
				&& res
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|desc_rhs| desc == desc_rhs)
		})?;

		let res = entry
			.res
			.as_ref()
			.unwrap_or_else(|| panic!("Detected recursive dependency on dependency {desc:?}."));

		Some(downcast_userdata_arc(res.clone()))
	}

	pub fn keep_alive<D: ResourceDescriptor>(&mut self, desc: &D) {
		let _ = desc;
		todo!()
	}
}
