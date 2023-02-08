#![allow(dead_code)]

use crucible_util::lang::polyfill::{BuildHasherPoly, OptionPoly};
use geode::{Entity, OwnedEntity};
use hashbrown::{hash_map::RawEntryMut, HashMap};

use std::{any::type_name, cell::Ref, hash::Hash};

pub trait AssetDescriptor: 'static + Hash + Eq + Clone {
	type Context<'a>;
	type Asset: 'static;

	fn construct(&self, asset_mgr: &mut AssetManager, cx: Self::Context<'_>) -> Self::Asset;

	fn keep_alive(&self, asset_mgr: &mut AssetManager) {
		let _ = asset_mgr;
		// (no op)
	}
}

#[derive(Debug, Default)]
pub struct AssetManager {
	assets: HashMap<ReifiedDescriptor, Option<OwnedEntity>>,
}

#[derive(Debug)]
struct ReifiedDescriptor {
	hash: u64,
	desc: OwnedEntity,
}

impl AssetManager {
	pub fn load<D: AssetDescriptor>(
		&mut self,
		desc: &D,
		cx: D::Context<'_>,
	) -> Ref<'static, D::Asset> {
		// Hash the descriptor
		let desc_hash = self.assets.hasher().p_hash_one(desc);

		// Try to reuse an existing asset.
		if let Some(asset) = self.find_with_hash(desc, desc_hash) {
			return asset;
		}

		// Insert a stub to detect recursive dependency loading
		let RawEntryMut::Vacant(entry) = self.assets
			.raw_entry_mut()
			.from_hash(desc_hash, |_| false) else { // We already know nothing can match this key.
				unreachable!();
			};

		let (desc_ent, desc_ent_ref) = Entity::new()
			.with_debug_label("asset descriptor")
			.with(desc.clone())
			.split_guard();

		entry.insert_with_hasher(
			desc_hash,
			ReifiedDescriptor {
				hash: desc_hash,
				desc: desc_ent,
			},
			None,
			|desc| desc.hash,
		);

		// Load the resource
		let res = desc.construct(self, cx);
		let res = OwnedEntity::new().with_debug_label("asset").with(res);
		let res_ref = res.get();

		// Write it!
		let RawEntryMut::Occupied(mut entry) = self.assets
			.raw_entry_mut()
			.from_hash(desc_hash, |v| v.hash == desc_hash && v.desc.entity() == desc_ent_ref) else {
				unreachable!();
			};

		entry.insert(Some(res));
		res_ref
	}

	pub fn find<D: AssetDescriptor>(&self, desc: &D) -> Option<Ref<'static, D::Asset>> {
		let desc_hash = self.assets.hasher().p_hash_one(desc);
		self.find_with_hash(desc, desc_hash)
	}

	fn find_with_hash<D: AssetDescriptor>(
		&self,
		desc: &D,
		desc_hash: u64,
	) -> Option<Ref<'static, D::Asset>> {
		self.assets
			.raw_entry()
			.from_hash(desc_hash, |v| {
				v.hash == desc_hash && v.desc.try_get::<D>().p_is_some_and(|v| &*v == desc)
			})
			.map(|(_, v)| {
				v.as_ref()
					.unwrap_or_else(|| {
						panic!(
							"attempted to acquire asset of type \"{}\", which was in the process of
							 loading",
							type_name::<D>()
						)
					})
					.get()
			})
	}

	pub fn keep_alive<D: AssetDescriptor>(&mut self, desc: &D) {
		let _ = desc;
		todo!()
	}
}
