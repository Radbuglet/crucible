use std::{
	borrow::Borrow,
	cell::UnsafeCell,
	collections::hash_map::RandomState,
	fmt, hash,
	ops::{Deref, DerefMut},
};

use derive_where::derive_where;
use hashbrown::{
	hash_map,
	raw::{Bucket, RawIter, RawTable},
	HashMap, TryReserveError,
};

use crate::lang::{
	polyfill::BuildHasherPoly,
	std_traits::{CellLike, TransparentCellLike},
};

use super::{
	drop_guard::{DropGuard, DropGuardHandler},
	ptr::unchecked_unify,
};

// === AutoHashMap === //

pub struct AutoHashMapBuilder<S = RandomState, P = DefaultForgetPolicy> {
	hasher: Option<S>,
	forget_policy: Option<P>,
	capacity: Option<usize>,
}

impl AutoHashMapBuilder {
	pub fn new() -> Self {
		Self {
			hasher: None,
			forget_policy: None,
			capacity: None,
		}
	}
}

impl<S, P> AutoHashMapBuilder<S, P> {
	pub fn with_hasher<S2>(self, hasher: S2) -> AutoHashMapBuilder<S2, P> {
		AutoHashMapBuilder {
			hasher: Some(hasher),
			forget_policy: self.forget_policy,
			capacity: self.capacity,
		}
	}

	pub fn with_forget_policy<P2>(self, policy: P2) -> AutoHashMapBuilder<S, P2> {
		AutoHashMapBuilder {
			hasher: self.hasher,
			forget_policy: Some(policy),
			capacity: self.capacity,
		}
	}

	pub fn with_capacity(self, capacity: usize) -> AutoHashMapBuilder<S, P> {
		AutoHashMapBuilder {
			hasher: self.hasher,
			forget_policy: self.forget_policy,
			capacity: Some(capacity),
		}
	}

	pub fn build<K, V>(self) -> AutoHashMap<K, V, S, P> {
		let hash_builder = self
			.hasher
			.unwrap_or_else(|| unsafe { unchecked_unify(RandomState::default()) });

		let policy = self
			.forget_policy
			.unwrap_or_else(|| unsafe { unchecked_unify(DefaultForgetPolicy) });

		let raw_map = match self.capacity {
			Some(capacity) => HashMap::with_capacity_and_hasher(capacity, hash_builder),
			None => HashMap::with_hasher(hash_builder),
		};

		AutoHashMap { policy, raw_map }
	}
}

#[derive_where(Debug; K: fmt::Debug, V: fmt::Debug, P: fmt::Debug)]
#[derive_where(Default; S: Default, P: Default)]
#[derive_where(Eq; K: Eq + hash::Hash, V: Eq, S: hash::BuildHasher)]
#[derive_where(PartialEq; K: Eq + hash::Hash, V: PartialEq, S: hash::BuildHasher)]
#[derive(Clone)]
pub struct AutoHashMap<K, V, S = RandomState, P = DefaultForgetPolicy> {
	#[derive_where(skip)]
	pub policy: P,
	pub raw_map: HashMap<K, V, S>,
}

impl<K, V> AutoHashMap<K, V> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self {
		HashMap::with_capacity_and_hasher(capacity, RandomState::default()).into()
	}
}

impl<K, V, S, P> AutoHashMap<K, V, S, P> {
	pub fn builder() -> AutoHashMapBuilder {
		AutoHashMapBuilder::new()
	}

	pub fn capacity(&self) -> usize {
		self.raw_map.capacity()
	}

	pub fn keys(&self) -> hash_map::Keys<K, V> {
		self.raw_map.keys()
	}

	pub fn into_keys(self) -> hash_map::IntoKeys<K, V> {
		self.raw_map.into_keys()
	}

	pub fn values(&self) -> hash_map::Values<'_, K, V> {
		self.raw_map.values()
	}

	pub fn into_values(self) -> hash_map::IntoValues<K, V> {
		self.raw_map.into_values()
	}

	pub fn iter(&self) -> hash_map::Iter<K, V> {
		self.raw_map.iter()
	}

	pub fn len(&self) -> usize {
		self.raw_map.len()
	}

	pub fn is_empty(&self) -> bool {
		self.raw_map.is_empty()
	}

	pub fn drain(&mut self) -> hash_map::Drain<K, V> {
		self.raw_map.drain()
	}

	pub fn clear(&mut self) {
		self.raw_map.clear()
	}

	pub fn hasher(&self) -> &S {
		self.raw_map.hasher()
	}
}

impl<K, V, S, P> AutoHashMap<K, V, S, P>
where
	P: ForgetPolicy<V>,
{
	pub fn retain<F>(&mut self, mut f: F)
	where
		F: FnMut(&K, &mut V) -> bool,
	{
		self.raw_map.retain(|k, v| {
			if f(k, v) {
				self.policy.is_alive(v)
			} else {
				false
			}
		})
	}

	pub fn iter_mut(&mut self) -> AutoMapIterMut<K, V, P> {
		let table = self.raw_map.raw_table();
		let iter = unsafe { table.iter() };

		AutoMapIterMut {
			policy: &self.policy,
			table: UnsafeCell::from_mut(table),
			iter,
		}
	}

	pub fn values_mut(&mut self) -> AutoMapValuesMut<'_, K, V, P> {
		AutoMapValuesMut(self.iter_mut())
	}
}

impl<K, V, S, P> AutoHashMap<K, V, S, P>
where
	K: Eq + hash::Hash,
	S: hash::BuildHasher,
	P: ForgetPolicy<V>,
{
	pub fn reserve(&mut self, additional: usize) {
		self.raw_map.reserve(additional)
	}

	pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
		self.raw_map.try_reserve(additional)
	}

	pub fn shrink_to_fit(&mut self) {
		self.raw_map.shrink_to_fit()
	}

	pub fn shrink_to(&mut self, min_capacity: usize) {
		self.raw_map.shrink_to(min_capacity)
	}

	// pub fn entry(&mut self, key: K) -> hash_map::Entry<K, V> {
	// 	todo!()
	// }

	pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		self.raw_map.get(k)
	}

	pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)>
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		self.raw_map.get_key_value(k)
	}

	pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		self.raw_map.contains_key(k)
	}

	pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<AutoRef<K, V, P>>
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		let hash = self.raw_map.hasher().p_hash_one(k);
		let table = self.raw_map.raw_table();
		let bucket = table.find(hash, |(k2, _)| k2.borrow() == k)?;

		Some(AutoRef {
			guard: DropGuard::new(
				AutoRefInner {
					policy: &self.policy,
					table: UnsafeCell::from_mut(table),
					bucket,
				},
				AutoRefDropHandler,
			),
		})
	}

	pub fn insert(&mut self, k: K, v: V) -> Option<V> {
		self.raw_map.insert(k, v)
	}

	pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		self.raw_map.remove(k)
	}

	pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
	where
		K: Borrow<Q>,
		Q: hash::Hash + Eq,
	{
		self.raw_map.remove_entry(k)
	}
}

impl<K, V, S, P: Default> From<HashMap<K, V, S>> for AutoHashMap<K, V, S, P> {
	fn from(map: HashMap<K, V, S>) -> Self {
		Self {
			policy: P::default(),
			raw_map: map,
		}
	}
}

// impl<'a, K, V, S, P> Extend<(&'a K, &'a V)> for AutoMap<K, V, S, P> {
// 	fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
// 		todo!()
// 	}
// }

// impl<'a, K, V, S, P> Extend<(K, V)> for AutoMap<K, V, S, P> {
// 	fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
// 		todo!()
// 	}
// }

// impl<'a, K, V, S, P, const N: usize> From<[(K, V); N]> for AutoMap<K, V, S, P> {
// 	fn from(value: [(K, V); N]) -> Self {
// 		todo!()
// 	}
// }

// impl<'a, K, V, S, P> FromIterator<(K, V)> for AutoMap<K, V, S, P> {
// 	fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
// 		todo!()
// 	}
// }

impl<'a, K, V, S, P> IntoIterator for &'a AutoHashMap<K, V, S, P> {
	type Item = (&'a K, &'a V);
	type IntoIter = hash_map::Iter<'a, K, V>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl<'a, K, V, S, P> IntoIterator for &'a mut AutoHashMap<K, V, S, P>
where
	P: ForgetPolicy<V>,
{
	type Item = (&'a K, AutoRef<'a, K, V, P>);
	type IntoIter = AutoMapIterMut<'a, K, V, P>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}

// === AutoMapIterMut === //

pub struct AutoMapIterMut<'a, K, V, P> {
	policy: &'a P,
	table: &'a UnsafeCell<RawTable<(K, V)>>,
	iter: RawIter<(K, V)>,
}

impl<'a, K: 'a, V: 'a, P: ForgetPolicy<V>> Iterator for AutoMapIterMut<'a, K, V, P> {
	type Item = (&'a K, AutoRef<'a, K, V, P>);

	fn next(&mut self) -> Option<Self::Item> {
		let bucket = self.iter.next()?;

		let key = unsafe { &(*bucket.as_ptr()).0 };

		Some((
			key,
			AutoRef {
				guard: DropGuard::new(
					AutoRefInner {
						policy: self.policy,
						table: self.table,
						bucket,
					},
					AutoRefDropHandler,
				),
			},
		))
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iter.size_hint()
	}
}

impl<'a, K: 'a, V: 'a, P: ForgetPolicy<V>> ExactSizeIterator for AutoMapIterMut<'a, K, V, P> {
	fn len(&self) -> usize {
		self.iter.len()
	}
}

// === AutoMapValueMut === //

pub struct AutoMapValuesMut<'a, K, V, P>(AutoMapIterMut<'a, K, V, P>);

impl<'a, K: 'a, V: 'a, P: ForgetPolicy<V>> Iterator for AutoMapValuesMut<'a, K, V, P> {
	type Item = AutoRef<'a, K, V, P>;

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next().map(|(_, v)| v)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.0.size_hint()
	}
}

impl<'a, K: 'a, V: 'a, P: ForgetPolicy<V>> ExactSizeIterator for AutoMapValuesMut<'a, K, V, P> {
	fn len(&self) -> usize {
		self.0.len()
	}
}

// === AutoRef === //

pub struct AutoRef<'a, K, V, P: ForgetPolicy<V>> {
	guard: DropGuard<AutoRefInner<'a, K, V, P>, AutoRefDropHandler>,
}

struct AutoRefInner<'a, K, V, P: ForgetPolicy<V>> {
	policy: &'a P,
	table: &'a UnsafeCell<RawTable<(K, V)>>,
	bucket: Bucket<(K, V)>,
}

struct AutoRefDropHandler;

impl<'a, K, V, P: ForgetPolicy<V>> AutoRef<'a, K, V, P> {
	pub fn defuse(me: Self) -> &'a mut V {
		&mut unsafe { DropGuard::defuse(me.guard).bucket.as_mut() }.1
	}
}

impl<K, V, P: ForgetPolicy<V>> Deref for AutoRef<'_, K, V, P> {
	type Target = V;

	fn deref(&self) -> &Self::Target {
		unsafe { &(*self.guard.bucket.as_ptr()).1 }
	}
}

impl<K, V, P: ForgetPolicy<V>> DerefMut for AutoRef<'_, K, V, P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut (*self.guard.bucket.as_ptr()).1 }
	}
}

impl<'a, K, V, P: ForgetPolicy<V>> DropGuardHandler<AutoRefInner<'a, K, V, P>>
	for AutoRefDropHandler
{
	fn destruct(self, inner: AutoRefInner<'a, K, V, P>) {
		if !inner
			.policy
			.is_alive(unsafe { &mut inner.bucket.as_mut().1 })
		{
			unsafe {
				inner.table.get_mut_unchecked().remove(inner.bucket);
			}
		}
	}
}

// === ForgetPolicy === //

pub trait ForgetPolicy<V> {
	fn is_alive(&self, value: &mut V) -> bool;
}

pub trait CanForget {
	fn is_alive(&self) -> bool;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct DefaultForgetPolicy;

impl<V: CanForget> ForgetPolicy<V> for DefaultForgetPolicy {
	fn is_alive(&self, value: &mut V) -> bool {
		value.is_alive()
	}
}

#[derive(Debug, Copy, Clone, Default)]
pub struct EmptyForgetPolicy;

impl<T> ForgetPolicy<T> for EmptyForgetPolicy
where
	for<'a> &'a T: IntoIterator,
{
	fn is_alive(&self, value: &mut T) -> bool {
		value.into_iter().next().is_some()
	}
}
