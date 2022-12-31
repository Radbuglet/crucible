use std::{
	borrow::Borrow,
	collections::hash_map::RandomState,
	hash::{self, BuildHasher},
	mem,
	ops::{Index, IndexMut},
	sync::Once,
};

use derive_where::derive_where;
use hashbrown::HashMap;
use parking_lot::Mutex;

use super::ptr::HeapPointerExt;

#[derive(Debug)]
#[derive_where(Default; S: Default)]
pub struct EventualMap<K, V: ?Sized, S = RandomState> {
	established: HashMap<K, Box<V>, S>,
	nursery: Mutex<HashMap<K, NurseryCell<V>, S>>,
}

#[derive(Debug)]
struct NurseryCell<V: ?Sized> {
	once: Box<Once>,
	value: Option<Box<V>>,
}

impl<K, V: ?Sized> EventualMap<K, V> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<K, V, S> EventualMap<K, V, S>
where
	K: hash::Hash + Eq + Copy,
	V: ?Sized,
	S: Default + BuildHasher,
{
	pub fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		Q: ?Sized + hash::Hash + Eq,
		K: Borrow<Q>,
	{
		if let Some(established) = self.established.get(key) {
			return Some(&established);
		}

		let nursery_map = self.nursery.lock();
		let value = nursery_map.get(key)?.value.as_ref()?;

		Some(unsafe {
			// Safety: the box is not going to be destroyed until the next mutating call to
			// `EventualMap` and this box exhibits exterior mutability w.r.t. the container.
			value.prolong_heap_ref()
		})
	}

	pub fn get_or_create<F>(&self, key: K, f: F) -> &V
	where
		F: FnOnce() -> Box<V>,
	{
		// See if we can acquire the component directly from the established map.
		if let Some(established) = self.established.get(&key) {
			return &established;
		}

		// Otherwise, see if it's a component in the nursery and acquire it from there.
		let mut nursery_map = self.nursery.lock();

		match nursery_map.entry(key).or_insert_with(|| NurseryCell {
			once: Box::new(Once::new()),
			value: None,
		}) {
			NurseryCell {
				value: Some(value), ..
			} => {
				// The value was already initialized so let's return it.
				unsafe {
					// Safety: the box is not going to be destroyed until the next mutating call to
					// `EventualMap` and this box exhibits exterior mutability w.r.t. the container.
					value.prolong_heap_ref()
				}
			}
			NurseryCell { once, .. } => {
				// If it was in neither the established map, nor the nursery, it must be actively
				// initializing or in need of initialization. We handle non-racy initialization using
				// the boxed `Once` instance.
				//
				// However, we must release the `nursery_map` mutex during initialization lest recursive calls
				// to `get_or_create` deadlock, hence this lifetime prolongation.
				let once = unsafe {
					// Safety: same exact logic as above.
					once.prolong_heap_ref()
				};

				drop(nursery_map);

				// Call the initialization routine or wait for an existing invocation to finish.
				//
				// Note that, even if we're the thread that inserted the value into the map, we may
				// not be the one actually invoking the method. This is because of the `nursery_map`
				// mutex guard has been dropped so someone may have already observed our added entry
				// and taken it upon themselves to initialize the cell instead of us.
				let mut value_to_return = None;
				once.call_once(|| {
					// Oh good, nothing is locked right now so there's no chance of deadlock.
					let value = f();

					// Now, insert the value into the mutex...
					value_to_return = Some(unsafe {
						// Safety: same exact logic as above.
						value.prolong_heap_ref()
					});
					self.nursery.lock().get_mut(&key).unwrap().value = Some(value);
				});

				// This was either just initialized in `call_once` or
				value_to_return.unwrap_or_else(|| unsafe {
					// Safety: same exact logic as above.
					self.nursery
						.lock()
						.get_mut(&key)
						.unwrap()
						.value
						.as_ref()
						.unwrap()
						.prolong_heap_ref()
				})
			}
		}
	}

	pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		Q: ?Sized + hash::Hash + Eq,
		K: Borrow<Q>,
	{
		self.flush();
		self.established.get_mut(key).map(|b| &mut **b)
	}

	pub fn add(&self, key: K, value: Box<V>) -> &V {
		let mut created = false;
		let value = self.get_or_create(key, || {
			created = true;
			value
		});
		assert!(created);
		value
	}

	pub fn insert(&mut self, key: K, value: Box<V>) -> &mut V {
		self.established.entry(key).insert(value).into_mut()
	}

	pub fn remove<Q>(&mut self, key: &Q) -> Option<Box<V>>
	where
		Q: ?Sized + hash::Hash + Eq,
		K: Borrow<Q>,
	{
		self.flush();
		self.established.remove(key)
	}

	pub fn flush(&mut self) {
		self.established.extend(
			mem::replace(self.nursery.get_mut(), HashMap::default())
				.into_iter()
				.map(|(k, v)| (k, v.value.unwrap())),
		)
	}
}

impl<'a, K, V, S, Q> Index<&'a Q> for EventualMap<K, V, S>
where
	K: hash::Hash + Eq + Copy + Borrow<Q>,
	V: ?Sized,
	S: Default + BuildHasher,
	Q: ?Sized + Eq + hash::Hash,
{
	type Output = V;

	fn index(&self, key: &'a Q) -> &Self::Output {
		self.get(key).unwrap()
	}
}

impl<'a, K, V, S, Q> IndexMut<&'a Q> for EventualMap<K, V, S>
where
	K: hash::Hash + Eq + Copy + Borrow<Q>,
	V: ?Sized,
	S: Default + BuildHasher,
	Q: ?Sized + Eq + hash::Hash,
{
	fn index_mut(&mut self, key: &'a Q) -> &mut Self::Output {
		self.get_mut(key).unwrap()
	}
}
