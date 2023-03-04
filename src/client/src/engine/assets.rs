use bort::{storage, CompRef, Entity, OwnedEntity};
use crucible_util::lang::{polyfill::BuildHasherPoly, tuple::ToOwnedTupleEq};
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
	pub fn cache<K, L, R>(&mut self, args: K, loader: L) -> CompRef<R>
	where
		K: ToOwnedTupleEq,
		K::Owned: 'static,
		L: AssetLoaderFunc<Output = R>,
		R: 'static,
	{
		// Acquire relevant storages
		let args_storage = storage::<K::Owned>();
		let vals_storage = storage::<R>();

		// Get the loading closure's function pointer. This is a sound way to differentiate asset
		// types because:
		//
		// 1. No false negatives unless the compiler decides to produce a different specialized
		//    closure dependent on the invocation site. FIXME: That could actually happen...
		//
		// 2. No false positives unless the compiler manages to combine the two closures, which is
		//    only sound if the two closures have the same effect, in which case the behavior is
		//    still correct and actually more optimal. FIXME: There could be funky tampolining of
		//    `dyn FnOnce()` instances.
		//
		// We have to go through an `AssetLoaderFunc` indirection because `FnOnce::call_once` is not
		// yet namable on stable.
		let func = L::load as usize;

		// Now, hash the combination of the function pointer and the arguments.
		let hash = self.assets.hasher().p_hash_one(&(func, &args));

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
					return false
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
		let res = loader.load(self);
		let res_ent = OwnedEntity::new();
		vals_storage.insert(res_ent.entity(), res);
		let res_ref = vals_storage.get(res_ent.entity());

		// Insert it into the store
		let RawEntryMut::Occupied(mut entry) = self.assets
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

pub trait AssetLoaderFunc {
	type Output;

	fn load(self, assets: &mut AssetManager) -> Self::Output;
}

impl<F: FnOnce(&mut AssetManager) -> R, R> AssetLoaderFunc for F {
	type Output = R;

	fn load(self, assets: &mut AssetManager) -> Self::Output {
		(self)(assets)
	}
}
