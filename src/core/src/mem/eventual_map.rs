use std::{
	borrow::Borrow,
	collections::hash_map::RandomState,
	hash::{self, BuildHasher},
	mem,
	ops::{Index, IndexMut},
};

use derive_where::derive_where;
use hashbrown::HashMap;
use parking_lot::Mutex;

use super::ptr::PointeeCastExt;

#[derive(Debug)]
#[derive_where(Default; S: Default)]
pub struct EventualMap<K, V: ?Sized, S = RandomState> {
	established: HashMap<K, Box<V>, S>,
	new: Mutex<HashMap<K, Option<Box<V>>, S>>,
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
	// TODO: Implement `raw_get`

	pub fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		Q: ?Sized + hash::Hash + Eq,
		K: Borrow<Q>,
	{
		if let Some(established) = self.established.get(key) {
			return Some(&established);
		}

		self.new.lock().get(key).map(|inner| {
			let inner = inner.as_ref().unwrap_or_else(|| {
				panic!("Attempted to acquire auto value while it was being initialized.");
			});

			let inner = &**inner;

			unsafe {
				// Safety: the box is not going to be destroyed until the next mutating call to
				// `EventualMap` and, barring `Provider::get_raw`—which has special semantics—this box
				// is exterior mutably w.r.t. the container.
				inner.prolong()
			}
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

		// Otherwise, see if it's a new component and acquire it from there.
		if let Some(established) = self.new.lock().entry(key).or_insert(None) {
			let inner = &**established;
			return unsafe {
				// Safety: the box is not going to be destroyed until the next mutating call to
				// `EventualMap` and, barring `Provider::get_raw`—which has special semantics—this box
				// is exterior mutably w.r.t. the container.
				inner.prolong()
			};
		}

		// If both checks failed, create the value...
		let value = f();
		let inner = unsafe {
			// Safety: the box is not going to be destroyed until the next mutating call to
			// `EventualMap` and, barring `Provider::get_raw`—which has special semantics—this box
			// is exterior mutably w.r.t. the container.
			(&*value).prolong()
		};

		// ...and put it in the new map.
		// N.B. because we're transferring control to user code, we have to reborrow this lock.
		self.new.lock().insert(key, Some(value));

		inner
	}

	pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		Q: ?Sized + hash::Hash + Eq,
		K: Borrow<Q>,
	{
		self.flush();
		self.established.get_mut(key).map(|b| &mut **b)
	}

	pub fn create(&self, key: K, value: Box<V>) {
		let mut created = false;
		self.get_or_create(key, || {
			created = true;
			value
		});
		assert!(created);
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
			mem::replace(self.new.get_mut(), HashMap::default())
				.into_iter()
				.map(|(k, v)| (k, v.unwrap())),
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
