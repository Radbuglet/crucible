#![allow(dead_code)]

use crucible_util::{
	debug::userdata::{BoxedUserdata, ErasedUserdata, Userdata},
	lang::{
		loan::downcast_userdata_arc,
		polyfill::{BuildHasherPoly, OptionPoly},
	},
};

use hashbrown::HashMap;

use std::{hash::Hash, sync::Arc};

pub trait AssetDescriptor: 'static + Userdata + Hash + Eq + Clone {
	type Context<'a>;
	type Asset: Userdata;

	fn construct(&self, asset_mgr: &mut AssetManager, cx: Self::Context<'_>) -> Arc<Self::Asset>;

	fn keep_alive(&self, asset_mgr: &mut AssetManager) {
		let _ = asset_mgr;
		// (no op)
	}
}

#[derive(Debug, Default)]
pub struct AssetManager {
	assets: HashMap<ReifiedDescriptor, Option<Arc<dyn Userdata>>>,
	// TODO: Implement automatic cleanup
}

#[derive(Debug)]
struct ReifiedDescriptor {
	hash: u64,
	desc: BoxedUserdata,
}

impl AssetManager {
	pub fn load<D: AssetDescriptor>(&mut self, desc: &D, cx: D::Context<'_>) -> Arc<D::Asset> {
		// Try to reuse an existing asset.
		if let Some(asset) = self.find(desc) {
			return asset;
		}

		// Insert an unfinished asset stub into the registry. This is used to detect cyclic
		// asset dependencies.
		let hash = self.assets.hasher().p_hash_one(&desc);
		let reified_desc = ReifiedDescriptor {
			hash,
			desc: Box::new(desc.clone()),
		};

		self.assets
			.raw_table()
			.insert(hash, (reified_desc, None), |(desc, _)| desc.hash);

		// Construct the asset
		let asset = desc.construct(self, cx);

		// Update the stub to contain the asset.
		let (_, stub) = self
			.assets
			.raw_table()
			.get_mut(hash, |(candidate, _)| {
				candidate
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|desc_rhs| desc == desc_rhs)
			})
			.unwrap();

		*stub = Some(asset);

		// Convert it into an `Arc<D::Asset>`
		let asset = stub.as_ref().unwrap();
		downcast_userdata_arc(asset.clone())
	}

	pub fn find<D: AssetDescriptor>(&self, desc: &D) -> Option<Arc<D::Asset>> {
		let hash = self.assets.hasher().p_hash_one(desc);
		let (_, asset) = self.assets.raw_entry().from_hash(hash, |candidate| {
			candidate.hash == hash
				&& candidate
					.desc
					.try_downcast_ref::<D>()
					.p_is_some_and(|candidate| desc == candidate)
		})?;

		let asset = asset
			.as_ref()
			.unwrap_or_else(|| panic!("Detected recursive dependency on dependency {desc:?}."));

		Some(downcast_userdata_arc(asset.clone()))
	}

	pub fn keep_alive<D: AssetDescriptor>(&mut self, desc: &D) {
		let _ = desc;
		todo!()
	}
}
