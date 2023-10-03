use std::{any::type_name, hash::BuildHasher};

use bort::{storage, CompRef, Entity, OwnedEntity};
use crucible_util::lang::tuple::ToOwnedTupleEq;
use hashbrown::{hash_map::RawEntryMut, HashMap};

#[derive(Debug, Default)]
pub struct AssetManager {
	assets: HashMap<ReifiedKey, Option<OwnedEntity>>,
}

#[derive(Debug)]
struct ReifiedKey {
	hash: u64,
	func: usize,
	args: OwnedEntity,
}

impl AssetManager {
	fn closure_identifier<L: FnOnce(&mut Self) -> R, R>(
		loader: L,
		assets: &mut Self,
		dummy: fn(&str),
	) -> R {
		dummy(type_name::<L>());
		loader(assets)
	}

	pub fn cache<K, L, R>(&mut self, args: K, loader: L) -> CompRef<'static, R>
	where
		K: ToOwnedTupleEq,
		K::Owned: 'static,
		L: FnOnce(&mut Self) -> R,
		R: 'static,
	{
		// Acquire relevant storages
		let args_storage = storage::<K::Owned>();
		let vals_storage = storage::<R>();

		// Get a function pointer to a unique closure identifier. This identifier mixes in both the
		// type name of the closure, which depends on the calling function path, and the actual
		// closure behavior itself.
		//
		// False negatives: we are referring to a function pointer in the abstract so the optimizer
		// is not able to perform context-dependent optimizations that would result in creating a
		// different function given a different calling context since it has **no context**.
		//
		// False positives: the closure identifier mixes in both the type name of the closure (which
		// is usually unique per calling function path) and the actual behavior of the closure being
		// invoked statically.
		//
		// FIXME: there is a risk that the user passes in a function pointer instead of an actual
		//        function.
		let func = Self::closure_identifier::<L, R> as usize;

		// Now, hash the combination of the function pointer and the arguments.
		let hash = self.assets.hasher().hash_one((func, &args));

		// Now, fetch the asset...
		let entry = self.assets.raw_entry_mut().from_hash(hash, |candidate| {
			// Check hash
			if hash != candidate.hash {
				return false;
			}

			// Check function pointer
			if func != candidate.func {
				return false;
			}

			// See if the candidate has the appropriate arguments
			let Some(candidate_args) = args_storage.try_get(candidate.args.entity()) else {
				return false;
			};

			// Finally, ensure that they are equal
			args.is_eq_owned(&candidate_args)
		});

		let args_entity: Entity;
		match entry {
			// If the asset has a value, yield it.
			RawEntryMut::Occupied(entry) => {
				return vals_storage.get(
					entry // OccupiedEntry
						.get() // `&Option<OwnedEntity>`
						.as_ref() // Option<&OwnedEntity>`
						.expect("Cyclic cache dependency detected") // `&OwnedEntity`
						.entity(), // `Entity`
				);
			}
			// Otherwise, mark the entry as "loading"...
			RawEntryMut::Vacant(entry) => {
				let args_entity_guard = OwnedEntity::new();
				args_storage.insert(args_entity_guard.entity(), args.to_owned());
				args_entity = args_entity_guard.entity();
				entry.insert_with_hasher(
					hash,
					ReifiedKey {
						hash,
						func,
						args: args_entity_guard,
					},
					None,
					|k| k.hash,
				);
			}
		};

		// Load the asset
		let res = loader(self);
		let res_ent = OwnedEntity::new();
		vals_storage.insert(res_ent.entity(), res);
		let res_ref = vals_storage.get(res_ent.entity());

		// Insert it into the store
		let RawEntryMut::Occupied(mut entry) = self
			.assets
			.raw_entry_mut()
			.from_hash(hash, |candidate| candidate.args.entity() == args_entity)
		else {
			unreachable!()
		};

		entry.insert(Some(res_ent));

		// And return our reference
		res_ref
	}
}
