use futures::executor::block_on;
use hashbrown::raw::RawTable;
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::collections::hash_map::RandomState;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::hash::{BuildHasher, Hash};
use std::ops::Deref;
use std::ptr::NonNull;

//> Quotas
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum QuotaId {
	Typed(TypeId),
	Dynamic(u64),
}

impl QuotaId {
	pub fn unique() -> Self {
		use std::sync::atomic::{AtomicU64, Ordering};

		static GEN: AtomicU64 = AtomicU64::new(0);

		let gen = GEN.fetch_add(1, Ordering::Relaxed);
		debug_assert!(gen != u64::MAX);
		Self::Dynamic(gen)
	}

	pub fn typed<Q: StaticQuota>() -> Self {
		Self::Typed(TypeId::of::<Q>())
	}
}

pub trait StaticQuota: Sized + 'static {
	fn id() -> QuotaId {
		QuotaId::typed::<Self>()
	}
}

pub macro new_quota($(
    $vis:vis $name:ident;
)*) {$(
    $vis struct $name {
        _private: (),
    }

    impl QuotaMarker for $name {}
)*}

//> Resource manager
#[derive(Default)]
pub struct ResourceManager {
	assets: RawTable<(u64, Box<dyn Any>, NonNull<ResourceState<()>>)>,
	hasher: RandomState,
}

impl ResourceManager {
	pub fn new() -> Self {
		Default::default()
	}

	pub async fn try_load_async<D: ResourceDescriptor>(
		&mut self,
		desc: D,
	) -> Result<ResRef<D::Resource>, <D::Loader as ResourceLoader>::Error> {
		let desc_key = desc.key();

		// Try to find the resource in the cache.
		let hash = self.hasher.hash_one(&desc_key);
		let entry = self.assets.get(hash, |(other_hash, key, _)| {
			// Check the hashes.
			if hash != *other_hash {
				return false;
			}

			// Check the key for type equality
			let key = match (&*key).downcast_ref::<D>() {
				Some(key) => key,
				None => return false,
			};

			// Check the key for equality
			key.eq(&desc_key)
		});

		if let Some((_, _, ptr)) = entry {
			// Safety: because each "ResourceKey" has exactly one resource type with which it is
			// associated, the "ResourceState" with a "ResourceKey<Resource = T>" key stores a
			// resource of type "T".
			let mut ptr = ptr.cast::<ResourceState<D::Resource>>();

			// Increment reference count.
			unsafe { ptr.as_ref().inc_rc() };
			return Ok(ResRef { ptr });
		}

		// Load the resource.
		let resource = desc.decompose().await?;

		// TODO: Manage quotas.

		// Register it.
		let state = Box::new(ResourceState {
			refs: Cell::new(1),
			value: resource,
		});
		let ptr = NonNull::from(Box::leak(state));

		self.assets
			.insert(hash, (hash, Box::new(desc) as _, ptr), |(hash, _, _)| *hash);

		Ok(ResRef { ptr })
	}

	pub fn try_load<D: ResourceDescriptor>(
		&mut self,
		desc: D,
	) -> Result<ResRef<D::Resource>, D::LoadError> {
		block_on(self.try_load_async(desc))
	}

	pub fn load<D: ResourceDescriptor>(&mut self, desc: D) -> ResRef<D::Resource> {
		self.try_load(desc).unwrap()
	}
}

#[repr(C)]
struct ResourceState<T> {
	refs: Cell<usize>,
	value: T,
}

impl<T> ResourceState<T> {
	fn inc_rc(&self) {
		self.refs.set(
			self.refs
				.get()
				.checked_add(1)
				.expect("Created too many references to a resource!"),
		);
	}

	fn dec_ref(&self) {
		self.refs.set(self.refs.get() - 1);
	}
}

pub struct ResRef<T> {
	ptr: NonNull<ResourceState<T>>,
}

impl<T> ResRef<T> {
	fn value(&self) -> &ResourceState<T> {
		unsafe { self.ptr.as_ref() }
	}
}

impl<T: Debug> Debug for ResRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(self.value(), f)
	}
}

impl<T: Display> Display for ResRef<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self.value(), f)
	}
}

impl<T> Clone for ResRef<T> {
	fn clone(&self) -> Self {
		self.value().inc_rc();
		Self { ptr: self.ptr }
	}
}

impl<T> Deref for ResRef<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value().value
	}
}

impl<T> Drop for ResRef<T> {
	fn drop(&mut self) {
		self.value().dec_ref();
	}
}

pub trait ResourceDescriptor {
	type Resource: Resource;
	type Key: ResourceKey<Resource = Self::Resource>;
	type LoadFuture: Future<Output = Result<Self::Resource, Self::LoadError>>;
	type LoadError: Error;

	fn key(&self) -> Self::Key;
}

pub trait ResourceKey: 'static + Sized + Hash + Eq {
	type Resource: Resource;
}

pub trait Resource {
	type SizeIter: Iterator<Item = (QuotaId, u64)>;

	fn size(&self) -> Self::SizeIter;
}
